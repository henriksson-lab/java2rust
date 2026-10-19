use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::facts::FactSet;
use crate::java_ir::{Block, Expr, JavaType, Member, MethodDecl, Program, Stmt, TypeDecl};
use crate::java_signature::{resolved_source_method_decl, source_method_rust_name};
use crate::signature::MethodKey;

/// Declaration-scoped callback representation, including source forwarding.
/// Escape evidence propagates backwards over forwarding edges once; no method
/// body is repeatedly rescanned to reach a global fixed point.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SourceCallbackPlan {
    inline: HashSet<(MethodKey, usize)>,
    invoked: HashSet<(MethodKey, usize)>,
    retained: HashSet<(MethodKey, usize)>,
    /// Virtual slots keep a native functional value ABI even if one known
    /// implementation only invokes the argument. This is not retention evidence.
    value_carrier: HashSet<(MethodKey, usize)>,
    resolved_owners: HashMap<MethodKey, Option<MethodKey>>,
}

impl SourceCallbackPlan {
    pub(crate) fn build(program: &Program, facts: &FactSet) -> Self {
        let mut types = Vec::new();
        collect_types(&program.types, &mut types);
        let mut slots = Vec::new();
        let mut indices = HashMap::new();
        let mut resolved_owners: HashMap<MethodKey, Option<MethodKey>> = HashMap::new();
        for &owner in &types {
            for member in &owner.members {
                let Member::Method(method) = member else {
                    continue;
                };
                for (index, param) in method.params.iter().enumerate() {
                    if !is_functional_slot(&param.ty, facts) {
                        continue;
                    }
                    let key = (MethodKey::source_method(owner, method), index);
                    indices.insert(key, slots.len());
                    slots.push((owner, method, index));
                }
            }
        }
        if slots.is_empty() {
            return Self::default();
        }
        let callback_methods = slots
            .iter()
            .map(|(owner, method, _)| MethodKey::source_method(owner, method))
            .collect::<HashSet<_>>();
        let rust_names = slots
            .iter()
            .map(|(_, method, _)| source_method_rust_name(method))
            .collect::<HashSet<_>>();
        for &owner in &types {
            for member in &owner.members {
                let Member::Method(method) = member else {
                    continue;
                };
                let Some(body) = &method.body else {
                    continue;
                };
                crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
                    let Expr::Call {
                        resolved: Some(call),
                        ..
                    } = expr
                    else {
                        return;
                    };
                    if !rust_names.contains(&call.rust_name) {
                        return;
                    }
                    if let Some((target_owner, target_method)) =
                        resolved_source_method_decl(program, &owner.name, call)
                    {
                        let target = MethodKey::source_method(target_owner, target_method);
                        if !callback_methods.contains(&target) {
                            return;
                        }
                        resolved_owners
                            .entry(MethodKey::resolved_call(call))
                            .and_modify(|prior| {
                                if prior.as_ref() != Some(&target) {
                                    *prior = None;
                                }
                            })
                            .or_insert(Some(target));
                    }
                });
            }
        }
        // Inherited shells and lexical body replay share the declaring slot's
        // callback ABI. Reuse the hierarchy resolver, including its override
        // and overload selection, instead of guessing an owner alias.
        for owner in types {
            if owner.extends.is_none() && owner.implements.is_empty() {
                continue;
            }
            for inherited in
                crate::inherited_members::inherited_instance_method_refs_for_type(owner, program)
            {
                let canonical =
                    MethodKey::source_method(inherited.declaring_ty(), inherited.method());
                if !callback_methods.contains(&canonical) {
                    continue;
                }
                let alias = MethodKey::rust_identity(&owner.name, inherited.rust_name());
                if !callback_methods.contains(&alias) {
                    resolved_owners.entry(alias).or_insert(Some(canonical));
                }
            }
        }
        let mut inline = Vec::with_capacity(slots.len());
        let mut invoked = Vec::with_capacity(slots.len());
        let mut retained = Vec::with_capacity(slots.len());
        let virtual_methods = crate::exception_plan::override_groups_for_program(program)
            .into_iter()
            .flatten()
            .filter_map(|key| {
                key.method_identity_parts()
                    .map(|(owner, name)| MethodKey::rust_identity(owner, name))
            })
            .collect::<HashSet<_>>();
        let mut value_carrier = slots
            .iter()
            .map(|(owner, method, _)| {
                method.body.is_none()
                    || (owner.kind == crate::java_ir::TypeKind::Interface
                        && !method.modifiers.contains(&crate::java_ir::Modifier::Static))
                    || virtual_methods.contains(&MethodKey::source_method(owner, method))
            })
            .collect::<Vec<_>>();
        let mut predecessors = vec![Vec::new(); slots.len()];
        for (source_index, (owner, method, index)) in slots.iter().enumerate() {
            let targets = RefCell::new(HashSet::new());
            let forwarding = |expr: &Expr, argument: usize| {
                let Expr::Call {
                    resolved: Some(call),
                    ..
                } = expr
                else {
                    return false;
                };
                let Some((target_owner, target_method)) =
                    resolved_source_method_decl(program, &owner.name, call)
                else {
                    return false;
                };
                let key = (
                    MethodKey::source_method(target_owner, target_method),
                    argument,
                );
                let Some(target_index) = indices.get(&key) else {
                    return false;
                };
                targets.borrow_mut().insert(*target_index);
                true
            };
            let (safe, direct_invocation) =
                callback_parameter_use(method, &method.params[*index].name, Some(&forwarding))
                    .unwrap_or((false, false));
            inline.push(safe);
            invoked.push(direct_invocation);
            retained.push(method_stores_priority_queue_comparator_callback(
                method,
                &method.params[*index].name,
            ));
            for target in targets.into_inner() {
                predecessors[target].push(source_index);
            }
        }
        propagate_callback_evidence(&mut inline, &predecessors, false);
        propagate_callback_evidence(&mut invoked, &predecessors, true);
        propagate_callback_evidence(&mut retained, &predecessors, true);
        propagate_callback_evidence(&mut value_carrier, &predecessors, true);
        let mut plan = Self {
            resolved_owners,
            ..Self::default()
        };
        for (((((owner, method, index), safe), may_invoke), is_retained), value_abi) in slots
            .into_iter()
            .zip(inline)
            .zip(invoked)
            .zip(retained)
            .zip(value_carrier)
        {
            let key = (MethodKey::source_method(owner, method), index);
            // A concrete method that cannot use the inline FnMut ABI emits
            // the native functional value, just like a virtual contract.
            // Record that ABI explicitly so callers do not fall back to a
            // bare lambda merely because the method is non-virtual.
            let value_abi = value_abi || !safe;
            if safe && !value_abi {
                plan.inline.insert(key.clone());
            }
            if value_abi {
                plan.value_carrier.insert(key.clone());
            }
            if may_invoke {
                plan.invoked.insert(key.clone());
            }
            if is_retained {
                plan.retained.insert(key);
            }
        }
        plan
    }

    pub(crate) fn is_inline(&self, owner: &TypeDecl, method: &MethodDecl, index: usize) -> bool {
        self.inline_at(MethodKey::source_method(owner, method), index)
    }

    /// Only a proven non-escaping source contract can rule out execution.
    /// Unknown/retained callbacks keep conservative effect handling.
    pub(crate) fn invocation_for_call(
        &self,
        call: &crate::java_ir::ResolvedCall,
        index: usize,
    ) -> Option<bool> {
        let key = self.canonical_key(MethodKey::resolved_call(call))?;
        if self.retained.contains(&(key.clone(), index)) {
            return None;
        }
        self.inline
            .contains(&(key.clone(), index))
            .then(|| self.invoked.contains(&(key, index)))
    }

    fn canonical_key(&self, key: MethodKey) -> Option<MethodKey> {
        match self.resolved_owners.get(&key) {
            Some(canonical) => canonical.clone(),
            None => Some(key),
        }
    }

    pub(crate) fn callable_key_for_method(
        &self,
        owner: &str,
        rust_name: &str,
        index: usize,
    ) -> Option<crate::usage::CallableKey> {
        self.callable_key_at(MethodKey::rust_identity(owner, rust_name), index)
    }

    pub(crate) fn callable_key_for_call(
        &self,
        call: &crate::java_ir::ResolvedCall,
        index: usize,
    ) -> Option<crate::usage::CallableKey> {
        self.callable_key_at(MethodKey::resolved_call(call), index)
    }

    fn callable_key_at(&self, key: MethodKey, index: usize) -> Option<crate::usage::CallableKey> {
        let key = self.canonical_key(key)?;
        (self.inline.contains(&(key.clone(), index))
            || self.value_carrier.contains(&(key.clone(), index)))
        .then(|| crate::usage::CallableKey::CallbackParameter {
            owner: Box::new(key.callable_key()),
            index,
        })
    }

    pub(crate) fn uses_value_carrier(&self, key: &crate::usage::CallableKey) -> bool {
        let crate::usage::CallableKey::CallbackParameter { owner, index } = key else {
            return false;
        };
        owner
            .method_identity_parts()
            .and_then(|(owner, name)| self.canonical_key(MethodKey::rust_identity(owner, name)))
            .is_some_and(|method| self.value_carrier.contains(&(method, *index)))
    }

    pub(crate) fn retains_for_method(&self, owner: &str, rust_name: &str, index: usize) -> bool {
        self.canonical_key(MethodKey::rust_identity(owner, rust_name))
            .is_some_and(|key| self.retained.contains(&(key, index)))
    }

    pub(crate) fn retains_callback(&self, key: &crate::usage::CallableKey) -> bool {
        let crate::usage::CallableKey::CallbackParameter { owner, index } = key else {
            return false;
        };
        owner
            .method_identity_parts()
            .is_some_and(|(owner, method)| {
                self.retained
                    .contains(&(MethodKey::rust_identity(owner, method), *index))
            })
    }

    fn inline_at(&self, key: MethodKey, index: usize) -> bool {
        self.canonical_key(key)
            .is_some_and(|key| self.inline.contains(&(key, index)))
    }
}

/// Both escape and execution evidence flow backwards through the same source
/// forwarding graph. Each fact changes once; recursive SCCs need no rescans.
fn propagate_callback_evidence(values: &mut [bool], predecessors: &[Vec<usize>], evidence: bool) {
    let mut work = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (*value == evidence).then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(target) = work.pop_front() {
        for &source in &predecessors[target] {
            if values[source] != evidence {
                values[source] = evidence;
                work.push_back(source);
            }
        }
    }
}

fn collect_types<'a>(types: &'a [TypeDecl], out: &mut Vec<&'a TypeDecl>) {
    for owner in types {
        out.push(owner);
        for member in &owner.members {
            if let Member::Type(nested) = member {
                collect_types(std::slice::from_ref(nested), out);
            }
        }
    }
}

fn is_functional_slot(ty: &JavaType, facts: &FactSet) -> bool {
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    if crate::call_plan::java_type_owner_is_parsed_source(
        name,
        &crate::types::TypeEnv::new(),
        facts,
    ) {
        return false;
    }
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let Some(shape) = crate::stdlib::functional_interface_shape(&canonical) else {
        return false;
    };
    shape
        .inputs()
        .iter()
        .copied()
        .chain(shape.returns())
        .all(|source| {
            source
                .type_argument_index()
                .is_none_or(|index| args.get(index).is_some())
        })
}

#[cfg(test)]
pub(crate) fn method_param_is_nonescaping_functional_callback(
    method: &MethodDecl,
    param_name: &str,
) -> bool {
    method_param_is_nonescaping_functional_callback_with_forwarding(method, param_name, None)
}

/// A source-call contract may prove that a bare callback argument is forwarded
/// without escaping. Unknown calls, captures, returns and stores still use the
/// same conservative escape rules as direct callback parameters.
#[cfg(test)]
pub(crate) fn method_param_is_nonescaping_functional_callback_with_forwarding(
    method: &MethodDecl,
    param_name: &str,
    forwarding: Option<&dyn Fn(&Expr, usize) -> bool>,
) -> bool {
    callback_parameter_use(method, param_name, forwarding).is_some_and(|(safe, _)| safe)
}

fn callback_parameter_use(
    method: &MethodDecl,
    param_name: &str,
    forwarding: Option<&dyn Fn(&Expr, usize) -> bool>,
) -> Option<(bool, bool)> {
    let Some(callback_method) = method.params.iter().find_map(|param| {
        (param.name == param_name && is_functional_slot(&param.ty, &FactSet::default()))
            .then(|| functional_interface_method_name(&param.ty))
            .flatten()
    }) else {
        return None;
    };
    let Some(body) = &method.body else {
        return None;
    };
    let mut saw_callback_call = false;
    let mut escaped = false;
    scan_block_for_callback_param(
        body,
        param_name,
        callback_method,
        &mut saw_callback_call,
        &mut escaped,
        forwarding,
    );
    // An unused callback also stays within the call's lifetime. Requiring an
    // invocation conflated representation with execution and left these slots
    // as opaque facade objects instead of accepting ordinary Java lambdas.
    Some((!escaped, saw_callback_call))
}

/// The single abstract method a functional-interface parameter is called through.
fn functional_interface_method_name(java_ty: &JavaType) -> Option<&'static str> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(name.as_str());
    crate::stdlib::functional_interface_shape(simple).map(|shape| shape.method())
}

fn scan_block_for_callback_param(
    block: &crate::java_ir::Block,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
    forwarding: Option<&dyn Fn(&Expr, usize) -> bool>,
) {
    for stmt in &block.statements {
        scan_stmt_for_callback_param(
            stmt,
            param_name,
            callback_method,
            saw_callback_call,
            escaped,
            forwarding,
        );
    }
}

fn scan_stmt_for_callback_param(
    stmt: &crate::java_ir::Stmt,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
    forwarding: Option<&dyn Fn(&Expr, usize) -> bool>,
) {
    match stmt {
        crate::java_ir::Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        crate::java_ir::Stmt::Assign { target, value } => {
            scan_expr_for_callback_param(
                target,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_expr_for_callback_param(
                value,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::Expr(expr)
        | crate::java_ir::Stmt::Return(Some(expr))
        | crate::java_ir::Stmt::Throw(expr) => {
            scan_expr_for_callback_param(
                expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_block_for_callback_param(
                then_block,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            if let Some(block) = else_block {
                scan_block_for_callback_param(
                    block,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        crate::java_ir::Stmt::While {
            condition, body, ..
        }
        | crate::java_ir::Stmt::DoWhile {
            condition, body, ..
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                scan_stmt_for_callback_param(
                    stmt,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            if let Some(expr) = condition {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            for expr in update {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::EnhancedFor { iterable, body, .. } => {
            scan_expr_for_callback_param(
                iterable,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::Switch { selector, cases } => {
            scan_expr_for_callback_param(
                selector,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        scan_expr_for_callback_param(
                            expr,
                            param_name,
                            callback_method,
                            saw_callback_call,
                            escaped,
                            forwarding,
                        );
                    }
                }
                for stmt in &case.statements {
                    scan_stmt_for_callback_param(
                        stmt,
                        param_name,
                        callback_method,
                        saw_callback_call,
                        escaped,
                        forwarding,
                    );
                }
            }
        }
        crate::java_ir::Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                scan_stmt_for_callback_param(
                    stmt,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            for catch in catches {
                scan_block_for_callback_param(
                    &catch.body,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            if let Some(block) = finally_block {
                scan_block_for_callback_param(
                    block,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        crate::java_ir::Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        crate::java_ir::Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            scan_block_for_callback_param(
                block,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        crate::java_ir::Stmt::Return(None)
        | crate::java_ir::Stmt::LocalType(_)
        | crate::java_ir::Stmt::Break(_)
        | crate::java_ir::Stmt::Continue(_)
        | crate::java_ir::Stmt::Unsupported(_) => {}
    }
}

fn scan_expr_for_callback_param(
    expr: &Expr,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
    forwarding: Option<&dyn Fn(&Expr, usize) -> bool>,
) {
    match expr {
        Expr::Var(name) if name == param_name => *escaped = true,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if matches!(receiver.as_ref(), Expr::Var(receiver_name) if receiver_name == param_name)
            // The interface's own single abstract method, from
            // `functional_interface_shape` -- not a list of names kept beside
            // it. The list held six and stopped at `Comparator::compare`, so a
            // comparator parameter was never recognised as a callback and stayed
            // an opaque struct that `.compare(..)` could not be called on.
            && name.as_str() == callback_method =>
        {
            *saw_callback_call = true;
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } if native_collections_sort_consumes_callback_param(
            receiver.as_deref(),
            name,
            args,
            param_name,
        ) =>
        {
            *saw_callback_call = true;
            if let Some(receiver) = receiver {
                scan_expr_for_callback_param(
                    receiver,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            if let Some(target) = args.first() {
                scan_expr_for_callback_param(
                    target,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            scan_expr_for_callback_param(
                receiver,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        Expr::Binary { left, right, .. } => {
            scan_expr_for_callback_param(
                left,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_expr_for_callback_param(
                right,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_expr_for_callback_param(
                then_expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            scan_expr_for_callback_param(
                else_expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
        }
        Expr::Switch { selector, cases } => {
            scan_expr_for_callback_param(
                selector,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
                forwarding,
            );
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        scan_expr_for_callback_param(
                            expr,
                            param_name,
                            callback_method,
                            saw_callback_call,
                            escaped,
                            forwarding,
                        );
                    }
                }
                scan_expr_for_callback_param(
                    &case.value,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                scan_expr_for_callback_param(
                    receiver,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            for (index, arg) in args.iter().enumerate() {
                if matches!(arg, Expr::Var(name) if name == param_name)
                    && forwarding.is_some_and(|accepts| accepts(expr, index))
                {
                    // Execution flows from the target contract, not from the
                    // act of forwarding this reference.
                } else {
                    scan_expr_for_callback_param(
                        arg,
                        param_name,
                        callback_method,
                        saw_callback_call,
                        escaped,
                        forwarding,
                    );
                }
            }
        }
        Expr::New { ty, args, body, .. }
            if priority_queue_constructor_consumes_callback_param(ty, args, param_name) =>
        {
            *saw_callback_call = true;
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                scan_expr_for_callback_param(
                    qualifier,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            if body
                .as_ref()
                .is_some_and(|members| members_capture_callback_param(members, param_name))
            {
                *escaped = true;
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    scan_expr_for_callback_param(
                        expr,
                        param_name,
                        callback_method,
                        saw_callback_call,
                        escaped,
                        forwarding,
                    );
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for expr in values {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                    forwarding,
                );
            }
        }
        Expr::Lambda { body, .. } => {
            if lambda_body_captures_callback_param(body, param_name) {
                *escaped = true;
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

fn members_capture_callback_param(members: &[Member], param_name: &str) -> bool {
    members.iter().any(|member| match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_references_callback_param(expr, param_name)),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|body| block_references_callback_param(body, param_name)),
        Member::Constructor(ctor) => ctor
            .body
            .as_ref()
            .is_some_and(|body| block_references_callback_param(body, param_name)),
        Member::Type(_) | Member::Unsupported { .. } => false,
    })
}

fn lambda_body_captures_callback_param(
    body: &crate::java_ir::LambdaBody,
    param_name: &str,
) -> bool {
    match body {
        crate::java_ir::LambdaBody::Expr(expr) => expr_references_callback_param(expr, param_name),
        crate::java_ir::LambdaBody::Block(block) => {
            block_references_callback_param(block, param_name)
        }
    }
}

fn block_references_callback_param(block: &crate::java_ir::Block, param_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_references_callback_param(stmt, param_name))
}

fn stmt_references_callback_param(stmt: &crate::java_ir::Stmt, param_name: &str) -> bool {
    match stmt {
        crate::java_ir::Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_references_callback_param(expr, param_name)),
        crate::java_ir::Stmt::Assign { target, value } => {
            expr_references_callback_param(target, param_name)
                || expr_references_callback_param(value, param_name)
        }
        crate::java_ir::Stmt::Expr(expr)
        | crate::java_ir::Stmt::Return(Some(expr))
        | crate::java_ir::Stmt::Throw(expr) => expr_references_callback_param(expr, param_name),
        crate::java_ir::Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_references_callback_param(condition, param_name)
                || block_references_callback_param(then_block, param_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_references_callback_param(block, param_name))
        }
        crate::java_ir::Stmt::While {
            condition, body, ..
        }
        | crate::java_ir::Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_references_callback_param(condition, param_name)
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_references_callback_param(stmt, param_name))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_references_callback_param(expr, param_name))
                || update
                    .iter()
                    .any(|expr| expr_references_callback_param(expr, param_name))
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::EnhancedFor { iterable, body, .. } => {
            expr_references_callback_param(iterable, param_name)
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::Switch { selector, cases } => {
            expr_references_callback_param(selector, param_name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_references_callback_param(expr, param_name)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_references_callback_param(stmt, param_name))
                })
        }
        crate::java_ir::Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|resource| stmt_references_callback_param(resource, param_name))
                || block_references_callback_param(body, param_name)
                || catches
                    .iter()
                    .any(|catch| block_references_callback_param(&catch.body, param_name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_references_callback_param(block, param_name))
        }
        crate::java_ir::Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_references_callback_param(expr, param_name)),
        crate::java_ir::Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_references_callback_param(block, param_name)
        }
        crate::java_ir::Stmt::Return(None)
        | crate::java_ir::Stmt::LocalType(_)
        | crate::java_ir::Stmt::Break(_)
        | crate::java_ir::Stmt::Continue(_)
        | crate::java_ir::Stmt::Unsupported(_) => false,
    }
}

pub(crate) fn expr_references_callback_param(expr: &Expr, param_name: &str) -> bool {
    match expr {
        Expr::Var(name) => name == param_name,
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_references_callback_param(receiver, param_name)
        }
        Expr::Binary { left, right, .. } => {
            expr_references_callback_param(left, param_name)
                || expr_references_callback_param(right, param_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_references_callback_param(condition, param_name)
                || expr_references_callback_param(then_expr, param_name)
                || expr_references_callback_param(else_expr, param_name)
        }
        Expr::Switch { selector, cases } => {
            expr_references_callback_param(selector, param_name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_references_callback_param(expr, param_name)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || expr_references_callback_param(&case.value, param_name)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_references_callback_param(receiver, param_name))
                || args
                    .iter()
                    .any(|arg| expr_references_callback_param(arg, param_name))
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_references_callback_param(qualifier, param_name))
                || args
                    .iter()
                    .any(|arg| expr_references_callback_param(arg, param_name))
                || body
                    .as_ref()
                    .is_some_and(|members| members_capture_callback_param(members, param_name))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_references_callback_param(expr, param_name))
                || initializer.as_ref().is_some_and(|values| {
                    values
                        .iter()
                        .any(|expr| expr_references_callback_param(expr, param_name))
                })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_references_callback_param(expr, param_name)),
        Expr::Lambda { body, .. } => lambda_body_captures_callback_param(body, param_name),
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn native_collections_sort_consumes_callback_param(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    param_name: &str,
) -> bool {
    if name != "sort" || args.len() != 2 {
        return false;
    }
    let Some(Expr::Var(receiver_name)) = receiver else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Collections"
        && matches!(&args[1], Expr::Var(arg_name) if arg_name == param_name)
}

fn priority_queue_constructor_consumes_callback_param(
    ty: &JavaType,
    args: &[Expr],
    param_name: &str,
) -> bool {
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = ty else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == "PriorityQueue")
        && matches!(args, [Expr::Var(arg_name)] if arg_name == param_name)
}

pub(crate) fn method_stores_priority_queue_comparator_callback(
    method: &MethodDecl,
    param_name: &str,
) -> bool {
    method
        .body
        .as_ref()
        .is_some_and(|body| block_stores_priority_queue_comparator_callback(body, param_name))
}

fn block_stores_priority_queue_comparator_callback(block: &Block, param_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_stores_priority_queue_comparator_callback(stmt, param_name))
}

fn stmt_stores_priority_queue_comparator_callback(stmt: &Stmt, param_name: &str) -> bool {
    match stmt {
        Stmt::Return(Some(expr)) | Stmt::Expr(expr) | Stmt::Throw(expr) => {
            expr_stores_priority_queue_comparator_callback(expr, param_name)
        }
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_stores_priority_queue_comparator_callback(expr, param_name),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || block_stores_priority_queue_comparator_callback(then_block, param_name)
                || else_block.as_ref().is_some_and(|block| {
                    block_stores_priority_queue_comparator_callback(block, param_name)
                })
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            body, condition, ..
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || block_stores_priority_queue_comparator_callback(body, param_name)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_stores_priority_queue_comparator_callback(stmt, param_name))
                || condition.as_ref().is_some_and(|expr| {
                    expr_stores_priority_queue_comparator_callback(expr, param_name)
                })
                || update
                    .iter()
                    .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name))
                || block_stores_priority_queue_comparator_callback(body, param_name)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_stores_priority_queue_comparator_callback(block, param_name)
        }
        _ => false,
    }
}

pub(crate) fn expr_stores_priority_queue_comparator_callback(
    expr: &Expr,
    param_name: &str,
) -> bool {
    match expr {
        Expr::New {
            ty,
            qualifier,
            args,
            ..
        } => {
            priority_queue_constructor_consumes_callback_param(ty, args, param_name)
                || qualifier.as_ref().is_some_and(|qualifier| {
                    expr_stores_priority_queue_comparator_callback(qualifier, param_name)
                })
                || args
                    .iter()
                    .any(|arg| expr_stores_priority_queue_comparator_callback(arg, param_name))
        }
        Expr::Call { receiver, args, .. } => {
            receiver.as_ref().is_some_and(|receiver| {
                expr_stores_priority_queue_comparator_callback(receiver, param_name)
            }) || args
                .iter()
                .any(|arg| expr_stores_priority_queue_comparator_callback(arg, param_name))
        }
        Expr::Binary { left, right, .. } => {
            expr_stores_priority_queue_comparator_callback(left, param_name)
                || expr_stores_priority_queue_comparator_callback(right, param_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || expr_stores_priority_queue_comparator_callback(then_expr, param_name)
                || expr_stores_priority_queue_comparator_callback(else_expr, param_name)
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_stores_priority_queue_comparator_callback(receiver, param_name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name))
                || initializer.as_ref().is_some_and(|values| {
                    values.iter().any(|expr| {
                        expr_stores_priority_queue_comparator_callback(expr, param_name)
                    })
                })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concrete_stored_callback_forwarders_use_the_emitted_value_abi() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.function.Supplier;
class Keeper { Supplier<String> value; Keeper(Supplier<String> value) { this.value = value; } }
class CallbackForwarding {
    static Keeper wrap(Supplier<String> value) { return new Keeper(value); }
    static Keeper forward(Supplier<String> value) { return wrap(value); }
    static String invoke(Supplier<String> value) { return value.get(); }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[1];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            let key = plan
                .callable_key_for_method(&owner.name, &source_method_rust_name(method), 0)
                .expect("every callback ABI must have its canonical contract");
            assert_eq!(plan.is_inline(owner, method, 0), method.name == "invoke");
            assert_eq!(plan.uses_value_carrier(&key), method.name != "invoke");
        }
    }

    #[test]
    fn virtual_value_abi_propagates_without_claiming_retention() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/fixtures/virtual_callback_contracts.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let mut checked = 0;
        for owner in &resolved.program.types {
            if !matches!(
                owner.name.as_str(),
                "CallbackConsumer" | "InvokingConsumer" | "VirtualCallbackContracts"
            ) {
                continue;
            }
            for member in &owner.members {
                let Member::Method(method) = member else {
                    continue;
                };
                if !matches!(method.name.as_str(), "consume" | "forward") {
                    continue;
                }
                let key = plan
                    .callable_key_for_method(&owner.name, &source_method_rust_name(method), 0)
                    .expect(
                        "virtual declarations and their forwarders need value argument planning",
                    );
                assert!(plan.uses_value_carrier(&key), "{key:?}");
                assert!(!plan.is_inline(owner, method, 0), "{key:?}");
                assert!(
                    !plan.retains_callback(&key),
                    "ABI requirement is not retention evidence: {key:?}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 3);
    }

    #[test]
    fn execution_evidence_flows_from_invocations_not_forwarding_cycles() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.function.IntPredicate;
            class Routes {
                static boolean invoke(IntPredicate predicate) { return predicate.test(1); }
                static boolean forward(IntPredicate predicate) { return invoke(predicate); }
                static boolean unused(IntPredicate predicate) { return true; }
                static boolean unusedForward(IntPredicate predicate) { return unused(predicate); }
                static boolean cycleA(IntPredicate predicate) { return cycleB(predicate); }
                static boolean cycleB(IntPredicate predicate) { return cycleA(predicate); }
                static boolean probe() {
                    invoke(value -> true);
                    forward(value -> true);
                    unused(value -> true);
                    unusedForward(value -> true);
                    return cycleA(value -> true);
                }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let mut seen = HashSet::new();
        for member in &resolved.program.types[0].members {
            let Member::Method(method) = member else {
                continue;
            };
            crate::semantic_analysis::walk_block_exprs(
                method.body.as_ref().unwrap(),
                &mut |expr| {
                    let Expr::Call {
                        resolved: Some(call),
                        ..
                    } = expr
                    else {
                        return;
                    };
                    let expected = match call.rust_name.as_str() {
                        "invoke" | "forward" => true,
                        "unused" | "unused_forward" | "cycle_a" | "cycle_b" => false,
                        _ => return,
                    };
                    assert_eq!(
                        plan.invocation_for_call(call, 0),
                        Some(expected),
                        "{}",
                        call.rust_name
                    );
                    seen.insert(call.rust_name.clone());
                },
            );
        }
        assert_eq!(seen.len(), 6);
    }

    #[test]
    fn inherited_callback_shell_uses_the_declaring_contract() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.function.IntPredicate;
            class Base { boolean invoke(IntPredicate predicate) { return predicate.test(1); } }
            class Child extends Base {}
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let base = &resolved.program.types[0];
        let child = &resolved.program.types[1];
        let method = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "invoke" => Some(method),
                _ => None,
            })
            .unwrap();
        assert!(plan.is_inline(base, method, 0));
        assert!(plan.is_inline(child, method, 0));
    }

    #[test]
    fn source_functional_lookalike_keeps_source_object_contract() {
        let parsed = crate::parser::parse_java_source(
            r#"
            class IntPredicate { boolean test(int value) { return value > 0; } }
            class Use { static boolean invoke(IntPredicate predicate) { return predicate.test(1); } }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        assert!(plan.inline.is_empty());
    }

    #[test]
    fn forwarding_closure_propagates_escape_evidence_without_name_guessing() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.function.IntPredicate;
            class Routes {
                static IntPredicate stored;
                static boolean invoke(IntPredicate predicate) { return predicate.test(1); }
                static boolean forward(IntPredicate predicate) { return invoke(predicate); }
                static boolean twice(IntPredicate predicate) { return forward(predicate); }
                static boolean unused(IntPredicate predicate) { return true; }
                static void store(IntPredicate predicate) { stored = predicate; }
                static void escape(IntPredicate predicate) { store(predicate); }
                static void twiceEscape(IntPredicate predicate) { escape(predicate); }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let plan = SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            assert_eq!(
                plan.is_inline(owner, method, 0),
                matches!(
                    method.name.as_str(),
                    "invoke" | "forward" | "twice" | "unused"
                ),
                "{}",
                method.name
            );
        }
    }
}
