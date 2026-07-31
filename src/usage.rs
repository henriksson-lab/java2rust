use std::collections::{HashMap, HashSet};

use crate::facts::{ExceptionKind, FacadeSlot, FactSet, TrivialAccessorKind};
use crate::java_ir::{
    Block, ConstructorInvocationKind, Expr, JavaType, LambdaBody, Member, Modifier, Program,
    ResolvedCall, ResolvedConstructor, Stmt, SwitchLabel, TypeDecl,
};
use crate::java_signature::{JavaCallableKey, source_method_rust_name};
use crate::stdlib::is_content_read_method;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageFacts {
    pub methods: HashMap<UsageMethodKey, MethodUsageFacts>,
    pub methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>,
    pub methods_by_java_key: HashMap<JavaCallableKey, MethodUsageFacts>,
    pub constructors: HashMap<UsageConstructorKey, MethodUsageFacts>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallGraph {
    pub nodes: HashSet<CallableKey>,
    pub edges: Vec<CallGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableKey {
    Method {
        type_name: String,
        method_name: String,
    },
    Constructor {
        type_name: String,
        rust_name: String,
    },
    Lambda {
        owner: Box<CallableKey>,
        ordinal: usize,
    },
    AnonymousMethod {
        owner: Box<CallableKey>,
        ordinal: usize,
        method_name: String,
    },
    ExternalMethod {
        receiver_type: Option<String>,
        method_name: String,
        arg_count: usize,
    },
    ExternalConstructor {
        type_name: String,
        arg_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphEdge {
    pub caller: CallableKey,
    pub callee: CallableKey,
    pub kind: CallGraphEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallGraphEdgeKind {
    DirectMethodCall,
    ConstructorCall,
    ConstructorDelegationThis,
    ConstructorDelegationSuper,
    AnonymousClassConstructor,
    AnonymousClassMethod,
    LambdaBody,
    HandledExceptionBoundary,
    UnresolvedExternalMethodCall,
    UnresolvedExternalConstructorCall,
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

impl UsageFacts {
    pub fn source_method_usage(
        &self,
        ty: &TypeDecl,
        method: &crate::java_ir::MethodDecl,
    ) -> Option<&MethodUsageFacts> {
        self.methods_by_java_key
            .get(&JavaCallableKey::source_method(ty.name.clone(), method))
    }
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
        receiver_type: Option<JavaType>,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        name: String,
        receiver_type: Option<JavaType>,
        arg_count: usize,
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
    InstanceOfTarget {
        target_ty: JavaType,
    },
    Mutated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldUse {
    AssignedFromCall {
        field_name: String,
        name: String,
        receiver_type: Option<JavaType>,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        field_name: String,
        name: String,
        receiver_type: Option<JavaType>,
        arg_count: usize,
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
    pub receiver_is_self: bool,
    pub receiver_targets_self_field: bool,
    pub null_arg_indices: Vec<usize>,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorCallSiteUse {
    pub name: String,
    pub arg_count: usize,
    pub null_arg_indices: Vec<usize>,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedConstructor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedCallResultUse {
    pub name: String,
    pub arg_count: usize,
    pub receiver_type: Option<JavaType>,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResultBoundaryPolicy {
    pub name: String,
    pub arg_count: usize,
    pub receiver_type: Option<JavaType>,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedCall>,
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
    propagate_self_mutability_through_self_calls(&mut facts);
    facts
}

fn propagate_self_mutability_through_self_calls(facts: &mut UsageFacts) {
    loop {
        let mut mutating_methods = facts
            .methods
            .iter()
            .chain(facts.methods_by_rust_name.iter())
            .filter_map(|(key, usage)| {
                usage
                    .mutates_self
                    .then(|| (key.type_name.clone(), key.method_name.clone()))
            })
            .collect::<HashSet<_>>();
        mutating_methods.extend(facts.methods_by_java_key.iter().filter_map(|(key, usage)| {
            usage
                .mutates_self
                .then(|| (key.owner.clone(), key.java_name.clone()))
        }));
        let mut changed = false;
        for usage in facts
            .methods
            .values_mut()
            .chain(facts.methods_by_rust_name.values_mut())
            .chain(facts.methods_by_java_key.values_mut())
        {
            if usage.mutates_self {
                continue;
            }
            if usage.call_sites.iter().any(|call_site| {
                (call_site.receiver_is_self || call_site.receiver_targets_self_field)
                    && call_site.resolved.as_ref().is_some_and(|resolved| {
                        mutating_methods.contains(&(
                            resolved.declaring_type.clone(),
                            resolved.rust_name.clone(),
                        ))
                    })
            }) {
                usage.mutates_self = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

pub fn build_call_graph(program: &Program) -> CallGraph {
    build_call_graph_with_facts(program, &FactSet::default())
}

pub fn build_call_graph_with_facts(program: &Program, facts: &FactSet) -> CallGraph {
    let mut graph = CallGraph::default();
    for ty in &program.types {
        build_type_call_graph(ty, &mut graph, facts);
    }
    graph
}

fn analyze_type_usage(ty: &TypeDecl, facts: &mut UsageFacts, semantic_facts: &FactSet) {
    let internal_methods = internal_method_names(ty);
    let field_names = field_names(ty);
    let field_types = field_types(ty);
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
                            &field_types,
                            semantic_facts,
                        );
                    }
                }
                let rust_method_name = source_method_rust_name(method);
                let rust_key = UsageMethodKey {
                    type_name: ty.name.clone(),
                    method_name: rust_method_name,
                };
                let java_key = UsageMethodKey {
                    type_name: ty.name.clone(),
                    method_name: method.name.clone(),
                };
                facts
                    .methods_by_rust_name
                    .insert(rust_key, method_facts.clone());
                let rust_key = UsageMethodKey {
                    type_name: ty.name.clone(),
                    method_name: source_method_rust_name(method),
                };
                facts.methods.insert(rust_key, method_facts.clone());
                facts.methods_by_java_key.insert(
                    JavaCallableKey::source_method(ty.name.clone(), method),
                    method_facts.clone(),
                );
                facts.methods.insert(java_key, method_facts);
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
                            &field_types,
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

fn build_type_call_graph(ty: &TypeDecl, graph: &mut CallGraph, facts: &FactSet) {
    let internal_methods = all_method_names(ty);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let caller = CallableKey::Method {
                    type_name: ty.name.clone(),
                    method_name: source_method_rust_name(method),
                };
                graph.nodes.insert(caller.clone());
                if let Some(body) = &method.body {
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        caller,
                    );
                    collect_call_graph_edges_from_block(body, graph, &mut context, facts);
                }
            }
            Member::Constructor(ctor) => {
                let caller = CallableKey::Constructor {
                    type_name: ty.name.clone(),
                    rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                };
                graph.nodes.insert(caller.clone());
                if let Some(body) = &ctor.body {
                    let mut context = CallGraphBuildContext::new(
                        ty.name.clone(),
                        internal_methods.clone(),
                        caller,
                    );
                    collect_call_graph_edges_from_block(body, graph, &mut context, facts);
                }
            }
            Member::Type(nested) => build_type_call_graph(nested, graph, facts),
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct CallGraphBuildContext {
    current_type: String,
    internal_methods: HashSet<String>,
    current_callable: CallableKey,
    next_lambda_ordinal: usize,
    next_anonymous_ordinal: usize,
    handled_catch_types: Vec<JavaType>,
}

impl CallGraphBuildContext {
    fn new(
        current_type: String,
        internal_methods: HashSet<String>,
        current_callable: CallableKey,
    ) -> Self {
        Self {
            current_type,
            internal_methods,
            current_callable,
            next_lambda_ordinal: 0,
            next_anonymous_ordinal: 0,
            handled_catch_types: Vec::new(),
        }
    }

    fn child_for(&self, current_callable: CallableKey) -> Self {
        Self {
            current_type: self.current_type.clone(),
            internal_methods: self.internal_methods.clone(),
            current_callable,
            next_lambda_ordinal: 0,
            next_anonymous_ordinal: 0,
            handled_catch_types: Vec::new(),
        }
    }

    fn with_handled_exception_boundary(&self, catches: &[crate::java_ir::CatchClause]) -> Self {
        let mut context = self.clone();
        for catch in catches {
            for catch_ty in &catch.types {
                if !context.handled_catch_types.contains(catch_ty) {
                    context.handled_catch_types.push(catch_ty.clone());
                }
            }
        }
        context
    }

    fn in_handled_exception_boundary(&self) -> bool {
        !self.handled_catch_types.is_empty()
    }

    fn next_lambda(&mut self) -> CallableKey {
        let ordinal = self.next_lambda_ordinal;
        self.next_lambda_ordinal += 1;
        CallableKey::Lambda {
            owner: Box::new(self.current_callable.clone()),
            ordinal,
        }
    }

    fn next_anonymous_method(&mut self, method_name: String) -> CallableKey {
        let ordinal = self.next_anonymous_ordinal;
        self.next_anonymous_ordinal += 1;
        CallableKey::AnonymousMethod {
            owner: Box::new(self.current_callable.clone()),
            ordinal,
            method_name,
        }
    }
}

fn all_method_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method.name.clone()),
            _ => None,
        })
        .collect()
}

fn collect_call_graph_edges_from_block(
    block: &Block,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    for stmt in &block.statements {
        collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
    }
}

fn collect_call_graph_edges_from_stmt(
    stmt: &Stmt,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_call_graph_edges_from_expr(initializer, graph, context, facts);
            }
        }
        Stmt::Assign { target, value } => {
            collect_call_graph_edges_from_expr(target, graph, context, facts);
            collect_call_graph_edges_from_expr(value, graph, context, facts);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_call_graph_edges_from_expr(expr, graph, context, facts);
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_block(then_block, graph, context, facts);
            if let Some(else_block) = else_block {
                collect_call_graph_edges_from_block(else_block, graph, context, facts);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
            }
            if let Some(condition) = condition {
                collect_call_graph_edges_from_expr(condition, graph, context, facts);
            }
            for expr in update {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_call_graph_edges_from_expr(iterable, graph, context, facts);
            collect_call_graph_edges_from_block(body, graph, context, facts);
        }
        Stmt::Switch { selector, cases } => {
            collect_call_graph_edges_from_expr(selector, graph, context, facts);
            for case in cases {
                for stmt in &case.statements {
                    collect_call_graph_edges_from_stmt(stmt, graph, context, facts);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                collect_call_graph_edges_from_stmt(resource, graph, context, facts);
            }
            if catch_clauses_do_not_rethrow_checked_exceptions(catches, facts) {
                let mut try_context = context.with_handled_exception_boundary(catches);
                collect_call_graph_edges_from_block(body, graph, &mut try_context, facts);
            } else {
                collect_call_graph_edges_from_block(body, graph, context, facts);
            }
            for catch in catches {
                collect_call_graph_edges_from_block(&catch.body, graph, context, facts);
            }
            if let Some(finally_block) = finally_block {
                collect_call_graph_edges_from_block(finally_block, graph, context, facts);
            }
        }
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            let (callee, edge_kind) = match kind {
                ConstructorInvocationKind::This => (
                    resolved
                        .as_ref()
                        .map(constructor_key_from_resolved)
                        .unwrap_or_else(|| CallableKey::Constructor {
                            type_name: context.current_type.clone(),
                            rust_name: "new".to_string(),
                        }),
                    CallGraphEdgeKind::ConstructorDelegationThis,
                ),
                ConstructorInvocationKind::Super => (
                    resolved
                        .as_ref()
                        .map(constructor_key_from_resolved)
                        .unwrap_or_else(|| CallableKey::ExternalConstructor {
                            type_name: "super".to_string(),
                            arg_count: args.len(),
                        }),
                    CallGraphEdgeKind::ConstructorDelegationSuper,
                ),
            };
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Stmt::LocalType(ty) => build_type_call_graph(ty, graph, facts),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_call_graph_edges_from_block(block, graph, context, facts)
        }
    }
}

fn collect_call_graph_edges_from_expr(
    expr: &Expr,
    graph: &mut CallGraph,
    context: &mut CallGraphBuildContext,
    facts: &FactSet,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let (callee, edge_kind) =
                call_graph_method_target(receiver.as_deref(), name, args.len(), resolved, context);
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            if let Some(receiver) = receiver {
                collect_call_graph_edges_from_expr(receiver, graph, context, facts);
            }
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
        }
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => {
            let callee = resolved
                .as_ref()
                .map(constructor_key_from_resolved)
                .unwrap_or_else(|| CallableKey::ExternalConstructor {
                    type_name: java_type_call_graph_name(ty),
                    arg_count: args.len(),
                });
            let edge_kind = if body.is_some() {
                CallGraphEdgeKind::AnonymousClassConstructor
            } else if resolved.is_some() {
                CallGraphEdgeKind::ConstructorCall
            } else {
                CallGraphEdgeKind::UnresolvedExternalConstructorCall
            };
            add_call_graph_edge(graph, context, callee, edge_kind, facts);
            for arg in args {
                collect_call_graph_edges_from_expr(arg, graph, context, facts);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Method(method) = member {
                        let anonymous = context.next_anonymous_method(method.name.clone());
                        add_call_graph_edge(
                            graph,
                            context,
                            anonymous.clone(),
                            CallGraphEdgeKind::AnonymousClassMethod,
                            facts,
                        );
                        if let Some(body) = &method.body {
                            let mut child = context.child_for(anonymous);
                            collect_call_graph_edges_from_block(body, graph, &mut child, facts);
                        }
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => {
            let lambda = context.next_lambda();
            add_call_graph_edge(
                graph,
                context,
                lambda.clone(),
                CallGraphEdgeKind::LambdaBody,
                facts,
            );
            let mut child = context.child_for(lambda);
            match body {
                LambdaBody::Expr(expr) => {
                    collect_call_graph_edges_from_expr(expr, graph, &mut child, facts)
                }
                LambdaBody::Block(block) => {
                    collect_call_graph_edges_from_block(block, graph, &mut child, facts)
                }
            }
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_call_graph_edges_from_expr(receiver, graph, context, facts);
        }
        Expr::Binary { left, right, .. } => {
            collect_call_graph_edges_from_expr(left, graph, context, facts);
            collect_call_graph_edges_from_expr(right, graph, context, facts);
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_call_graph_edges_from_expr(expr, graph, context, facts);
        }
        Expr::ArrayAccess(array, index) => {
            collect_call_graph_edges_from_expr(array, graph, context, facts);
            collect_call_graph_edges_from_expr(index, graph, context, facts);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_call_graph_edges_from_expr(expr, graph, context, facts);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_call_graph_edges_from_expr(expr, graph, context, facts);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_call_graph_edges_from_expr(condition, graph, context, facts);
            collect_call_graph_edges_from_expr(then_expr, graph, context, facts);
            collect_call_graph_edges_from_expr(else_expr, graph, context, facts);
        }
        Expr::Switch { selector, cases } => {
            collect_call_graph_edges_from_expr(selector, graph, context, facts);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_call_graph_edges_from_expr(expr, graph, context, facts);
                    }
                }
                collect_call_graph_edges_from_expr(&case.value, graph, context, facts);
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn call_graph_method_target(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    resolved: &Option<ResolvedCall>,
    context: &CallGraphBuildContext,
) -> (CallableKey, CallGraphEdgeKind) {
    if let Some(resolved) = resolved {
        return (
            CallableKey::Method {
                type_name: resolved.declaring_type.clone(),
                method_name: resolved.rust_name.clone(),
            },
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    if context.internal_methods.contains(name)
        && receiver.is_none_or(|receiver| matches!(receiver, Expr::This))
    {
        return (
            CallableKey::Method {
                type_name: context.current_type.clone(),
                method_name: name.to_string(),
            },
            CallGraphEdgeKind::DirectMethodCall,
        );
    }
    (
        CallableKey::ExternalMethod {
            receiver_type: receiver.and_then(expr_call_graph_type_name),
            method_name: name.to_string(),
            arg_count,
        },
        CallGraphEdgeKind::UnresolvedExternalMethodCall,
    )
}

fn add_call_graph_edge(
    graph: &mut CallGraph,
    context: &CallGraphBuildContext,
    callee: CallableKey,
    kind: CallGraphEdgeKind,
    facts: &FactSet,
) {
    let caller = &context.current_callable;
    let kind = if context.in_handled_exception_boundary()
        && matches!(
            kind,
            CallGraphEdgeKind::DirectMethodCall
                | CallGraphEdgeKind::ConstructorCall
                | CallGraphEdgeKind::UnresolvedExternalMethodCall
                | CallGraphEdgeKind::UnresolvedExternalConstructorCall
        )
        && callable_checked_exceptions_are_caught(&callee, &context.handled_catch_types, facts)
    {
        CallGraphEdgeKind::HandledExceptionBoundary
    } else {
        kind
    };
    graph.nodes.insert(caller.clone());
    graph.nodes.insert(callee.clone());
    let edge = CallGraphEdge {
        caller: caller.clone(),
        callee,
        kind,
    };
    if !graph.edges.contains(&edge) {
        graph.edges.push(edge);
    }
}

fn catch_clauses_do_not_rethrow_checked_exceptions(
    catches: &[crate::java_ir::CatchClause],
    facts: &FactSet,
) -> bool {
    !catches.is_empty()
        && catches
            .iter()
            .all(|catch| block_does_not_rethrow_checked_exception(&catch.body, facts))
}

fn callable_checked_exceptions_are_caught(
    callee: &CallableKey,
    catch_types: &[JavaType],
    facts: &FactSet,
) -> bool {
    let checked = callable_checked_exception_types(callee, facts);
    !checked.is_empty()
        && checked
            .iter()
            .all(|thrown| exception_caught_by_any(thrown, catch_types, facts))
}

fn callable_checked_exception_types(callee: &CallableKey, facts: &FactSet) -> Vec<JavaType> {
    let thrown = match callee {
        CallableKey::Method {
            type_name,
            method_name,
        }
        | CallableKey::Constructor {
            type_name,
            rust_name: method_name,
        } => facts
            .method_effect_for(type_name, method_name)
            .map(|effect| effect.thrown.clone())
            .unwrap_or_default(),
        CallableKey::ExternalMethod {
            receiver_type,
            method_name,
            arg_count,
        } => facts
            .external_api_effect_facts
            .iter()
            .filter(|fact| {
                fact.method_name == *method_name
                    && fact.arg_count == Some(*arg_count)
                    && fact.receiver_type.as_ref() == receiver_type.as_ref()
            })
            .flat_map(|fact| fact.thrown.clone())
            .collect(),
        CallableKey::ExternalConstructor { .. }
        | CallableKey::Lambda { .. }
        | CallableKey::AnonymousMethod { .. } => Vec::new(),
    };
    thrown
        .into_iter()
        .filter(|ty| {
            java_type_name_for_usage(ty).is_some_and(|name| {
                facts.exception_kind_for(name) != Some(ExceptionKind::Unchecked)
            })
        })
        .collect()
}

fn exception_caught_by_any(thrown: &JavaType, catch_types: &[JavaType], facts: &FactSet) -> bool {
    catch_types
        .iter()
        .any(|catch_ty| exception_assignable_to_catch(thrown, catch_ty, facts))
}

fn exception_assignable_to_catch(thrown: &JavaType, catch_ty: &JavaType, facts: &FactSet) -> bool {
    let (Some(thrown_name), Some(catch_name)) = (
        java_type_name_for_usage(thrown),
        java_type_name_for_usage(catch_ty),
    ) else {
        return false;
    };
    if crate::facts::external_type_name_matches(thrown_name, catch_name)
        || matches!(
            catch_name,
            "Exception" | "java.lang.Exception" | "Throwable" | "java.lang.Throwable"
        )
    {
        return true;
    }
    facts
        .hierarchy_lookup_index()
        .hierarchy_subtype_policy(facts, thrown, catch_ty)
        .reaches
}

fn block_does_not_rethrow_checked_exception(block: &Block, facts: &FactSet) -> bool {
    !block
        .statements
        .iter()
        .any(|stmt| stmt_rethrows_checked_exception(stmt, facts))
}

fn stmt_rethrows_checked_exception(stmt: &Stmt, facts: &FactSet) -> bool {
    match stmt {
        Stmt::Throw(expr) => thrown_expr_is_checked_exception(expr, facts),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            !block_does_not_rethrow_checked_exception(block, facts)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            !block_does_not_rethrow_checked_exception(then_block, facts)
                || else_block
                    .as_ref()
                    .is_some_and(|block| !block_does_not_rethrow_checked_exception(block, facts))
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            !block_does_not_rethrow_checked_exception(body, facts)
                || catches
                    .iter()
                    .any(|catch| !block_does_not_rethrow_checked_exception(&catch.body, facts))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| !block_does_not_rethrow_checked_exception(block, facts))
        }
        _ => false,
    }
}

fn thrown_expr_is_checked_exception(expr: &Expr, facts: &FactSet) -> bool {
    let Some(type_name) = thrown_expr_type_name(expr) else {
        return true;
    };
    facts.exception_kind_for(&type_name) != Some(ExceptionKind::Unchecked)
}

fn thrown_expr_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::New { ty, .. } | Expr::Cast { ty, .. } => Some(java_type_call_graph_name(ty)),
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

fn java_type_name_for_usage(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => {
            None
        }
    }
}

fn constructor_key_from_resolved(resolved: &ResolvedConstructor) -> CallableKey {
    CallableKey::Constructor {
        type_name: resolved.declaring_type.clone(),
        rust_name: resolved.rust_name.clone(),
    }
}

fn expr_call_graph_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Cast { ty, .. } => Some(java_type_call_graph_name(ty)),
        Expr::ClassLiteral(ty) => Some(java_type_call_graph_name(ty)),
        Expr::Super => Some("super".to_string()),
        Expr::This => None,
        _ => None,
    }
}

fn java_type_call_graph_name(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(element) => format!("{}[]", java_type_call_graph_name(element)),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::Wildcard(_) => "?".to_string(),
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

fn field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
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
    field_types: &HashMap<String, JavaType>,
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
                if let Some(policy) = call_result_boundary_policy_with_context(
                    initializer,
                    internal_methods,
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                ) {
                    record_use(
                        facts,
                        name,
                        BindingUse::AssignedFromCall {
                            name: policy.name,
                            receiver_type: policy.receiver_type,
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
                    field_types,
                    semantic_facts,
                );
            }
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target {
                if is_unshadowed_field_name(facts, field_names, name) {
                    facts.mutates_self = true;
                    if let Some(policy) = call_result_boundary_policy_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
                        field_names,
                        field_types,
                        semantic_facts,
                    ) {
                        facts.field_uses.push(FieldUse::AssignedFromCall {
                            field_name: name.clone(),
                            name: policy.name,
                            receiver_type: policy.receiver_type,
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
                    if let Some(policy) = call_result_boundary_policy_with_context(
                        value,
                        internal_methods,
                        facts,
                        current_type,
                        field_names,
                        field_types,
                        semantic_facts,
                    ) {
                        record_use(
                            facts,
                            name,
                            BindingUse::AssignedFromCall {
                                name: policy.name,
                                receiver_type: policy.receiver_type,
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
                if let Some(policy) = call_result_boundary_policy_with_context(
                    value,
                    internal_methods,
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                ) {
                    facts.field_uses.push(FieldUse::AssignedFromCall {
                        field_name: field_name.clone(),
                        name: policy.name,
                        receiver_type: policy.receiver_type,
                        boundary: policy.boundary,
                    });
                }
                analyze_stored_expr(value, facts, field_name);
            }
            if array_assignment_targets_self_field(target, facts, field_names) {
                facts.mutates_self = true;
            }
            analyze_expr(
                target,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
            analyze_expr(
                value,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
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
            record_returned_call_result_use(
                expr,
                facts,
                internal_methods,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            );
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
                field_types,
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
                field_types,
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
                field_types,
                semantic_facts,
            );
            for stmt in &then_block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
                        field_types,
                        semantic_facts,
                    );
                }
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
                    field_types,
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
                    field_types,
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
                    field_types,
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
                    field_types,
                    semantic_facts,
                );
            }
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            analyze_expr(
                iterable,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
            let saved_binding = facts.bindings.remove(var_name);
            facts.bindings.insert(
                var_name.clone(),
                BindingUseFacts {
                    name: var_name.clone(),
                    java_ty: var_type.clone(),
                    uses: Vec::new(),
                },
            );
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                );
            }
            if let Some(saved_binding) = saved_binding {
                facts.bindings.remove(var_name);
                facts.bindings.insert(var_name.clone(), saved_binding);
            }
        }
        Stmt::Switch { selector, cases } => {
            analyze_expr(
                selector,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
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
                        field_types,
                        semantic_facts,
                    );
                }
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            for stmt in &block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
                    field_types,
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
                    field_types,
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
            field_types,
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
                null_arg_indices: null_argument_indices(args),
                boundary: policy.boundary,
                resolved: resolved.clone(),
            });
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    "constructor",
                    None,
                    args.len(),
                    arg_index,
                    policy.boundary,
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
    facts: &MethodUsageFacts,
) -> Option<CallResultBoundaryPolicy> {
    let empty_field_types = HashMap::new();
    let empty_facts = FactSet::default();
    call_result_boundary_policy_with_context(
        expr,
        internal_methods,
        facts,
        "",
        &HashSet::new(),
        &empty_field_types,
        &empty_facts,
    )
}

fn call_result_boundary_policy_with_context(
    expr: &Expr,
    internal_methods: &HashSet<String>,
    facts: &MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<CallResultBoundaryPolicy> {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let receiver_type = receiver.as_deref().and_then(|receiver| {
                call_result_receiver_type_with_context(
                    receiver,
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                )
            });
            let boundary =
                call_argument_boundary_policy(receiver.as_deref(), name, internal_methods).boundary;
            let boundary = resolved_receiver_call_boundary(
                boundary,
                receiver_type.as_ref(),
                resolved.as_ref(),
            );
            let boundary = modeled_receiver_call_boundary(
                boundary,
                receiver_type.as_ref(),
                name,
                args.len(),
                semantic_facts,
            );
            Some(CallResultBoundaryPolicy {
                name: name.clone(),
                arg_count: args.len(),
                boundary,
                receiver_type,
                resolved: resolved.clone(),
            })
        }
        Expr::New { resolved, .. } => Some(CallResultBoundaryPolicy {
            name: "new".to_string(),
            arg_count: 0,
            receiver_type: None,
            boundary: constructor_result_boundary_policy(resolved.as_ref()),
            resolved: None,
        }),
        _ => None,
    }
}

fn call_result_receiver_type_with_context(
    receiver: &Expr,
    facts: &MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    match receiver {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Expr::FieldAccess(inner, field_name) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
        _ => call_result_receiver_type(receiver, facts),
    }
}

fn call_result_receiver_type(receiver: &Expr, facts: &MethodUsageFacts) -> Option<JavaType> {
    match receiver {
        Expr::Var(name) => facts
            .bindings
            .get(name)
            .map(|binding| binding.java_ty.clone()),
        Expr::This | Expr::Super => None,
        Expr::Cast { ty, .. } => Some(ty.clone()),
        _ => None,
    }
}

fn analyze_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        internal_methods,
        field_names,
        field_types,
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
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        internal_methods,
        field_names,
        field_types,
        semantic_facts,
        true,
    );
}

fn analyze_nested_argument_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) {
    if matches!(expr, Expr::Var(_)) {
        return;
    }
    analyze_expr(
        expr,
        facts,
        current_type,
        internal_methods,
        field_names,
        field_types,
        semantic_facts,
    );
}

fn call_receiver_is_self(receiver: Option<&Expr>) -> bool {
    matches!(receiver, None | Some(Expr::This) | Some(Expr::Super))
}

fn null_argument_indices(args: &[Expr]) -> Vec<usize> {
    args.iter()
        .enumerate()
        .filter_map(|(index, arg)| {
            matches!(arg, Expr::Literal(crate::java_ir::Literal::Null)).then_some(index)
        })
        .collect()
}

fn analyze_expr_with_context(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    result_returned: bool,
) {
    if expr_mutates_self_field(expr, facts, field_names) {
        facts.mutates_self = true;
    }
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
                field_types,
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
            let receiver_type = call_receiver_java_type_with_context(
                receiver.as_deref(),
                facts,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            );
            let call_boundary = resolved_receiver_call_boundary(
                policy.boundary,
                receiver_type.as_ref(),
                resolved.as_ref(),
            );
            let call_boundary = modeled_receiver_call_boundary(
                call_boundary,
                receiver_type.as_ref(),
                name,
                args.len(),
                semantic_facts,
            );
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
                receiver_is_self: call_receiver_is_self(receiver.as_deref()),
                receiver_targets_self_field: receiver
                    .as_deref()
                    .is_some_and(|receiver| expr_targets_self_field(receiver, facts, field_names)),
                null_arg_indices: null_argument_indices(args),
                boundary: call_boundary,
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
                        boundary: call_boundary,
                    },
                );
            }
            if call_mut_borrow_arg_targets_self_field(
                receiver.as_deref(),
                name,
                args,
                facts,
                field_names,
                semantic_facts,
            ) {
                facts.mutates_self = true;
            }
            for (arg_index, arg) in args.iter().enumerate() {
                let arg_boundary =
                    call_argument_use_boundary_policy(name, arg_index, call_boundary);
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    name,
                    receiver_type.clone(),
                    args.len(),
                    arg_index,
                    arg_boundary,
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
                    field_types,
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
                field_types,
                semantic_facts,
            );
            analyze_expr(
                right,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
        }
        Expr::Cast { expr, .. } => analyze_expr_with_context(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
            result_returned,
        ),
        Expr::Unary { expr, .. } => analyze_expr(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
        ),
        Expr::InstanceOf { expr, ty, .. } => {
            if let Expr::Var(binding) = expr.as_ref() {
                record_use(
                    facts,
                    binding,
                    BindingUse::InstanceOfTarget {
                        target_ty: ty.clone(),
                    },
                );
            }
            analyze_expr(
                expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
        }
        Expr::ArrayAccess(array, index) => {
            analyze_expr(
                array,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
            analyze_expr(
                index,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    analyze_expr(
                        expr,
                        facts,
                        current_type,
                        internal_methods,
                        field_names,
                        field_types,
                        semantic_facts,
                    );
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
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
                field_types,
                semantic_facts,
            );
            analyze_expr_with_context(
                then_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                result_returned,
            );
            analyze_expr_with_context(
                else_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
                result_returned,
            );
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    field_types,
                    semantic_facts,
                    arg,
                    "new",
                    Some(ty.clone()),
                    args.len(),
                    arg_index,
                    constructor_argument_boundary_policy(resolved.as_ref(), arg_index),
                );
                analyze_nested_argument_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                );
            }
        }
        Expr::Lambda { params, body } => analyze_lambda_body(
            params,
            body,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
        ),
        Expr::Switch { selector, cases } => {
            analyze_expr(
                selector,
                facts,
                current_type,
                internal_methods,
                field_names,
                field_types,
                semantic_facts,
            );
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        analyze_expr(
                            expr,
                            facts,
                            current_type,
                            internal_methods,
                            field_names,
                            field_types,
                            semantic_facts,
                        );
                    }
                }
                analyze_expr(
                    &case.value,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                );
            }
        }
        Expr::Var(name) => record_use(facts, name, BindingUse::ContentRead),
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn analyze_lambda_body(
    params: &[crate::java_ir::LambdaParam],
    body: &LambdaBody,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) {
    let saved_params = params
        .iter()
        .map(|param| {
            let saved = facts.bindings.remove(&param.name);
            facts.bindings.insert(
                param.name.clone(),
                BindingUseFacts {
                    name: param.name.clone(),
                    java_ty: param
                        .ty
                        .clone()
                        .unwrap_or_else(|| JavaType::Unknown("lambda parameter".to_string())),
                    uses: Vec::new(),
                },
            );
            (param.name.clone(), saved)
        })
        .collect::<Vec<_>>();

    match body {
        LambdaBody::Expr(expr) => analyze_expr(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            field_types,
            semantic_facts,
        ),
        LambdaBody::Block(block) => {
            for stmt in &block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    field_types,
                    semantic_facts,
                );
            }
        }
    }

    for (name, saved) in saved_params {
        facts.bindings.remove(&name);
        if let Some(saved) = saved {
            facts.bindings.insert(name, saved);
        }
    }
}

fn record_returned_call_result_use(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    internal_methods: &HashSet<String>,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) {
    match expr {
        Expr::Call { .. } | Expr::New { .. } => {
            let Some(policy) = call_result_boundary_policy_with_context(
                expr,
                internal_methods,
                facts,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            ) else {
                return;
            };
            facts.returned_call_results.push(ReturnedCallResultUse {
                name: policy.name,
                arg_count: policy.arg_count,
                receiver_type: policy.receiver_type,
                boundary: policy.boundary,
                resolved: policy.resolved,
            });
        }
        Expr::Cast { expr, .. } => record_returned_call_result_use(
            expr,
            facts,
            internal_methods,
            current_type,
            field_names,
            field_types,
            semantic_facts,
        ),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            record_returned_call_result_use(
                then_expr,
                facts,
                internal_methods,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            );
            record_returned_call_result_use(
                else_expr,
                facts,
                internal_methods,
                current_type,
                field_names,
                field_types,
                semantic_facts,
            );
        }
        _ => {}
    }
}

fn record_call_arg_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    _field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
    arg: &Expr,
    call_name: &str,
    receiver_type: Option<JavaType>,
    arg_count: usize,
    arg_index: usize,
    boundary: CallArgumentBoundary,
) {
    if let Some(field_name) = trivial_getter_self_field(arg, current_type, semantic_facts) {
        facts.field_uses.push(FieldUse::PassedToCall {
            field_name,
            name: call_name.to_string(),
            receiver_type,
            arg_count,
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
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            });
        }
        Expr::Var(binding) => record_use(
            facts,
            binding,
            BindingUse::PassedToCall {
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            },
        ),
        Expr::FieldAccess(receiver, field_name) if matches!(receiver.as_ref(), Expr::This) => {
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: field_name.clone(),
                name: call_name.to_string(),
                receiver_type,
                arg_count,
                arg_index,
                boundary,
            });
        }
        _ => {}
    }
}

fn call_mut_borrow_arg_targets_self_field(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    usage: &MethodUsageFacts,
    field_names: &HashSet<String>,
    facts: &FactSet,
) -> bool {
    facts
        .facade_abi_facts
        .iter()
        .filter(|abi| abi.key.member.0 == name && abi.key.arity == args.len())
        .filter(|abi| {
            facade_abi_matches_call_receiver(abi.key.owner.type_name.as_str(), receiver, usage)
        })
        .any(|abi| {
            abi.args.iter().enumerate().any(|(index, slot)| {
                facade_slot_requires_mut_borrow(slot)
                    && args
                        .get(index)
                        .is_some_and(|arg| expr_targets_self_field(arg, usage, field_names))
            })
        })
}

fn facade_abi_matches_call_receiver(
    owner: &str,
    receiver: Option<&Expr>,
    usage: &MethodUsageFacts,
) -> bool {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return true;
    };
    if usage.bindings.contains_key(receiver_name) {
        return true;
    }
    crate::stdlib::canonical_external_receiver_type_key(receiver_name)
        == crate::stdlib::canonical_external_receiver_type_key(owner)
}

fn facade_slot_requires_mut_borrow(slot: &FacadeSlot) -> bool {
    match slot {
        FacadeSlot::MutBorrowed => true,
        FacadeSlot::Optional(inner) | FacadeSlot::Slice(inner) => {
            facade_slot_requires_mut_borrow(inner)
        }
        FacadeSlot::Owned
        | FacadeSlot::Borrowed
        | FacadeSlot::Index
        | FacadeSlot::DynTrait(_)
        | FacadeSlot::CrateHandle(_) => false,
    }
}

fn expr_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
        Expr::FieldAccess(receiver, _) => matches!(receiver.as_ref(), Expr::This),
        Expr::ArrayAccess(array, _) => expr_targets_self_field(array, facts, field_names),
        _ => false,
    }
}

fn call_receiver_java_type(receiver: Option<&Expr>, facts: &MethodUsageFacts) -> Option<JavaType> {
    let Some(Expr::Var(name)) = receiver else {
        return None;
    };
    facts
        .bindings
        .get(name)
        .map(|binding| binding.java_ty.clone())
        .or_else(|| {
            static_owner_receiver_name(name).map(|owner| JavaType::Unknown(owner.to_string()))
        })
}

fn call_receiver_java_type_with_context(
    receiver: Option<&Expr>,
    facts: &MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    match receiver {
        Some(Expr::Var(name)) if is_unshadowed_field_name(facts, field_names, name) => {
            source_field_type(current_type, name, field_types, semantic_facts)
        }
        Some(Expr::FieldAccess(inner, field_name)) if matches!(inner.as_ref(), Expr::This) => {
            source_field_type(current_type, field_name, field_types, semantic_facts)
        }
        _ => call_receiver_java_type(receiver, facts),
    }
}

fn source_field_type(
    current_type: &str,
    field_name: &str,
    field_types: &HashMap<String, JavaType>,
    semantic_facts: &FactSet,
) -> Option<JavaType> {
    field_types.get(field_name).cloned().or_else(|| {
        semantic_facts
            .external_field_for(current_type, field_name)
            .and_then(|field| field.field_type.clone())
    })
}

fn static_owner_receiver_name(name: &str) -> Option<&str> {
    let first = name.chars().next()?;
    if first.is_ascii_uppercase() {
        Some(name)
    } else {
        match crate::stdlib::canonical_external_receiver_type_key(name).as_str() {
            "Arrays" => Some("Arrays"),
            "Collections" => Some("Collections"),
            _ => None,
        }
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

fn array_assignment_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    let Expr::ArrayAccess(array, _) = expr else {
        return false;
    };
    array_expr_targets_self_field(array, facts, field_names)
}

fn array_expr_targets_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
        Expr::FieldAccess(receiver, _) => matches!(receiver.as_ref(), Expr::This),
        Expr::ArrayAccess(array, _) => array_expr_targets_self_field(array, facts, field_names),
        _ => false,
    }
}

fn expr_mutates_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            ..
        } => {
            array_assignment_targets_self_field(left, facts, field_names)
                || match left.as_ref() {
                    Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
                    target => target_is_this_field(target),
                }
        }
        Expr::Unary { expr, .. } => match expr.as_ref() {
            Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
            target => {
                target_is_this_field(target)
                    || array_assignment_targets_self_field(target, facts, field_names)
            }
        },
        Expr::Call {
            receiver: Some(receiver),
            name,
            ..
        } if is_mutating_collection_method_name(name) => match receiver.as_ref() {
            Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
            target => target_is_this_field(target),
        },
        _ => false,
    }
}

fn is_mutating_collection_method_name(name: &str) -> bool {
    matches!(
        name,
        "add"
            | "addAll"
            | "add_all"
            | "clear"
            | "put"
            | "putAll"
            | "put_all"
            | "remove"
            | "removeIf"
            | "remove_if"
            | "insert"
            | "extend"
            | "push"
            | "retain"
    )
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

fn constructor_result_boundary_policy(
    resolved: Option<&ResolvedConstructor>,
) -> CallArgumentBoundary {
    if resolved.is_some() {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    }
}

fn constructor_argument_boundary_policy(
    resolved: Option<&ResolvedConstructor>,
    arg_index: usize,
) -> CallArgumentBoundary {
    if resolved
        .and_then(|resolved| resolved.params.get(arg_index))
        .is_some()
    {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    }
}

fn resolved_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver_type: Option<&JavaType>,
    resolved: Option<&ResolvedCall>,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    }
    let Some(resolved) = resolved else {
        return boundary;
    };
    if resolved.kind != crate::java_ir::CallKind::Instance {
        return boundary;
    }
    if receiver_static_type_matches_declaring_type(receiver_type, &resolved.declaring_type) {
        CallArgumentBoundary::KnownContentRead
    } else {
        boundary
    }
}

fn modeled_receiver_call_boundary(
    boundary: CallArgumentBoundary,
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    semantic_facts: &FactSet,
) -> CallArgumentBoundary {
    if boundary != CallArgumentBoundary::Unknown {
        return boundary;
    }
    let Some(receiver_key) = external_receiver_type_key(receiver_type) else {
        return boundary;
    };
    if semantic_facts
        .external_api_effect_for_receiver_with_arity(Some(&receiver_key), name, Some(arg_count))
        .is_some_and(|effect| {
            effect.receiver_mutability == crate::facts::ReceiverMutabilityFact::ReadOnly
        })
    {
        return CallArgumentBoundary::KnownContentRead;
    }
    if crate::external_packs::external_api_effect_for_receiver_with_arity(
        Some(&receiver_key),
        name,
        Some(arg_count),
    )
    .is_some_and(|effect| {
        effect.receiver_mutability == crate::facts::ReceiverMutabilityFact::ReadOnly
    }) {
        return CallArgumentBoundary::KnownContentRead;
    }
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(&receiver_key);
    if matches!(
        receiver_simple,
        "Math" | "StrictMath" | "Integer" | "Float" | "Double" | "String" | "Objects"
    ) && crate::stdlib::known_external_method_return_type(Some(&receiver_key), name, arg_count)
        .is_some()
    {
        return CallArgumentBoundary::KnownContentRead;
    }
    boundary
}

fn external_receiver_type_key(receiver_type: Option<&JavaType>) -> Option<String> {
    match receiver_type? {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        _ => None,
    }
}

fn receiver_static_type_matches_declaring_type(
    receiver_type: Option<&JavaType>,
    declaring_type: &str,
) -> bool {
    let Some(JavaType::Named { name, .. }) = receiver_type else {
        return false;
    };
    name == declaring_type
        || name.rsplit('.').next() == Some(declaring_type)
        || declaring_type.rsplit('.').next() == Some(name)
}

pub fn call_argument_use_boundary_policy(
    name: &str,
    arg_index: usize,
    call_boundary: CallArgumentBoundary,
) -> CallArgumentBoundary {
    if crate::stdlib::is_content_read_argument_method(name, arg_index) {
        CallArgumentBoundary::KnownContentRead
    } else {
        call_boundary
    }
}

fn record_use(facts: &mut MethodUsageFacts, name: &str, usage: BindingUse) {
    if let Some(binding) = facts.bindings.get_mut(name) {
        binding.uses.push(usage);
    }
}
