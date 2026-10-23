//! Reduce proven private, contained enclosing-instance environments to final
//! field references. This is not escape analysis for arbitrary Java objects.
//! Unknown initialization, escape, lifetime or physical-storage contracts keep
//! the existing strong enclosing-instance representation.
use std::collections::{BTreeSet, HashSet};

use crate::facts::{
    Capture, CaptureFacts, CaptureSource, FactSet, FieldKey, OuterCapturedField,
    OuterFieldCapturePlan,
};
use crate::java_ir::{
    BinaryOp, Block, CallKind, ConstructorInvocationKind, Expr, FieldDecl, JavaType, Member,
    Modifier, Program, Stmt, TypeDecl, TypeKind, UnaryOp,
};
use crate::representation::RepresentationPlan;
use crate::rust_ir::RustType;
use crate::signature::SignaturePlan;
use crate::slot_shape::OwnershipShape;

fn ident(name: &str) -> String {
    crate::parser::to_snake_case(name)
}
fn static_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static)
}
fn private_final(field: &FieldDecl) -> bool {
    !static_field(field)
        && field.modifiers.contains(&Modifier::Private)
        && field.modifiers.contains(&Modifier::Final)
}
fn same_type(left: &str, right: &str) -> bool {
    crate::stdlib::canonical_external_receiver_type_key(left)
        == crate::stdlib::canonical_external_receiver_type_key(right)
}
fn new_is(expr: &Expr, inner: &TypeDecl) -> bool {
    matches!(expr, Expr::New { resolved: Some(call), .. }
        if same_type(&call.declaring_type, &inner.name))
}
fn occurrences(expr: &Expr, inner: &TypeDecl) -> usize {
    let mut count = 0;
    crate::semantic_analysis::walk_expr(expr, &mut |expr| {
        count += usize::from(new_is(expr, inner))
    });
    count
}

fn bound_names(ty: &TypeDecl, include_fields: bool) -> HashSet<String> {
    let mut names = HashSet::new();
    for member in &ty.members {
        let (params, body) = match member {
            Member::Field(field) if include_fields => {
                names.insert(ident(&field.name));
                continue;
            }
            Member::Method(method) => (&method.params, method.body.as_ref()),
            Member::Constructor(ctor) => (&ctor.params, ctor.body.as_ref()),
            _ => continue,
        };
        names.extend(params.iter().map(|p| ident(&p.name)));
        if let Some(body) = body {
            crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| match stmt {
                Stmt::VarDecl { name, .. } => {
                    names.insert(ident(name));
                }
                Stmt::EnhancedFor { var_name, .. } => {
                    names.insert(ident(var_name));
                }
                Stmt::Try { catches, .. } => names.extend(catches.iter().map(|c| ident(&c.name))),
                _ => {}
            });
            crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
                if let Expr::InstanceOf {
                    binding: Some(name),
                    ..
                } = expr
                {
                    names.insert(ident(name));
                }
            });
        }
    }
    names
}

struct Reads<'a> {
    fields: &'a [&'a FieldDecl],
    own: HashSet<String>,
    captured: BTreeSet<String>,
    captures: &'a CaptureFacts,
}
impl Reads<'_> {
    fn field(&self, name: &str) -> Option<&FieldDecl> {
        self.fields
            .iter()
            .copied()
            .find(|field| ident(&field.name) == ident(name))
    }
    fn writes_outer(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Var(name) if self.field(name).is_some_and(|f| !static_field(f)))
    }
    fn expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Var(name) => {
                if let Some(field) = self.field(name) {
                    if !static_field(field) {
                        if !private_final(field) {
                            return false;
                        }
                        self.captured.insert(ident(name));
                    }
                }
                true
            }
            Expr::This
            | Expr::Super
            | Expr::Lambda { .. }
            | Expr::MethodReference { .. }
            | Expr::Raw(_)
            | Expr::Unsupported { .. } => false,
            Expr::Literal(_) | Expr::ClassLiteral(_) => true,
            Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                self.own.contains(&ident(field))
            }
            Expr::FieldAccess(receiver, field) => field != "this" && self.expr(receiver),
            Expr::Call {
                receiver,
                args,
                resolved,
                ..
            } => {
                if receiver.is_none()
                    && !resolved
                        .as_ref()
                        .is_some_and(|call| call.kind == CallKind::Static)
                {
                    return false; // Implicit this/outer invocation requires the object.
                }
                receiver.as_ref().is_none_or(|r| self.expr(r)) && args.iter().all(|a| self.expr(a))
            }
            Expr::New {
                qualifier,
                args,
                body,
                resolved,
                ..
            } => {
                if qualifier.is_some()
                    || body.is_some()
                    || resolved.as_ref().is_none_or(|call| {
                        self.captures
                            .outer_this_type_for_type(&call.declaring_type)
                            .is_some()
                    })
                {
                    return false;
                }
                args.iter().all(|a| self.expr(a))
            }
            Expr::Binary { op, left, right } => {
                if matches!(op, BinaryOp::Assign) && self.writes_outer(left) {
                    return false;
                }
                self.expr(left) && self.expr(right)
            }
            Expr::Unary { op, expr } => {
                if matches!(
                    op,
                    UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec
                ) && self.writes_outer(expr)
                {
                    return false;
                }
                self.expr(expr)
            }
            Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => self.expr(expr),
            Expr::ArrayAccess(array, index) => self.expr(array) && self.expr(index),
            Expr::ArrayInitializer(values) => values.iter().all(|v| self.expr(v)),
            Expr::ArrayCreation {
                dimensions,
                initializer,
                ..
            } => {
                dimensions.iter().all(|v| self.expr(v))
                    && initializer
                        .as_ref()
                        .is_none_or(|values| values.iter().all(|v| self.expr(v)))
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => self.expr(condition) && self.expr(then_expr) && self.expr(else_expr),
            Expr::Switch { .. } => false,
        }
    }
    fn body(&mut self, body: &Block) -> bool {
        let mut valid = true;
        crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
            valid &= match stmt {
                Stmt::Assign { target, value } => {
                    !self.writes_outer(target) && self.expr(target) && self.expr(value)
                }
                Stmt::VarDecl { initializer, .. } => {
                    initializer.as_ref().is_none_or(|e| self.expr(e))
                }
                Stmt::Expr(e) | Stmt::Throw(e) => self.expr(e),
                Stmt::Return(e) => e.as_ref().is_none_or(|e| self.expr(e)),
                Stmt::If { condition, .. }
                | Stmt::While { condition, .. }
                | Stmt::DoWhile { condition, .. } => self.expr(condition),
                Stmt::For {
                    condition, update, ..
                } => {
                    condition.as_ref().is_none_or(|e| self.expr(e))
                        && update.iter().all(|e| self.expr(e))
                }
                Stmt::EnhancedFor { iterable, .. } => self.expr(iterable),
                Stmt::ConstructorInvocation {
                    kind: ConstructorInvocationKind::Super,
                    args,
                    ..
                } => args.is_empty(),
                Stmt::ConstructorInvocation { .. }
                | Stmt::LocalType(_)
                | Stmt::Unsupported(_)
                | Stmt::Switch { .. } => false,
                _ => true,
            };
        });
        valid
    }
}

/// A reference to a storage field may only be used as the receiver of a
/// method implemented by this non-self-publishing final strategy. Returning,
/// copying, passing or binding the strategy itself fails this proof.
fn storage_expr(expr: &Expr, storage: &HashSet<String>, methods: &HashSet<String>) -> bool {
    fn is_storage(expr: &Expr, storage: &HashSet<String>) -> bool {
        match expr {
            Expr::Var(name) => storage.contains(&ident(name)),
            Expr::FieldAccess(_, field) => storage.contains(&ident(field)),
            _ => false,
        }
    }
    match expr {
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if is_storage(receiver, storage) => {
            methods.contains(&ident(name))
                && args.iter().all(|arg| storage_expr(arg, storage, methods))
        }
        expr if is_storage(expr, storage) => false,
        _ => {
            let mut valid = true;
            // Validate direct children only: recursively visiting every node
            // would mistake an admitted receiver for an escaping value.
            let root = expr as *const Expr;
            crate::semantic_analysis::walk_expr(expr, &mut |child| {
                if child as *const Expr != root && is_storage(child, storage) {
                    valid = false;
                }
            });
            valid
        }
    }
}

struct Initialization<'a> {
    inner: &'a TypeDecl,
    fields: &'a [&'a FieldDecl],
    required: &'a BTreeSet<String>,
    storage: HashSet<String>,
    count: usize,
    bound: HashSet<String>,
}
impl Initialization<'_> {
    fn field(&self, target: &Expr) -> Option<&FieldDecl> {
        let name = match target {
            Expr::Var(name) if !self.bound.contains(&ident(name)) => name,
            Expr::FieldAccess(receiver, name) if matches!(receiver.as_ref(), Expr::This) => name,
            _ => return None,
        };
        self.fields
            .iter()
            .copied()
            .find(|f| ident(&f.name) == ident(name))
    }
    fn creation(&mut self, value: &Expr, initialized: &BTreeSet<String>) -> bool {
        if occurrences(value, self.inner) == 0 {
            return true;
        }
        match value {
            Expr::New {
                qualifier,
                args,
                body: None,
                ..
            } if new_is(value, self.inner) => {
                if !self.required.is_subset(initialized)
                    || qualifier
                        .as_ref()
                        .is_some_and(|q| !matches!(q.as_ref(), Expr::This))
                    || args.iter().any(|arg| occurrences(arg, self.inner) != 0)
                {
                    return false;
                }
                self.count += 1;
                true
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                occurrences(condition, self.inner) == 0
                    && self.creation(then_expr, initialized)
                    && self.creation(else_expr, initialized)
            }
            Expr::Cast { ty, expr } if !matches!(ty, JavaType::Primitive(_)) => {
                self.creation(expr, initialized)
            }
            _ => false,
        }
    }
    fn assign(&mut self, target: &Expr, value: &Expr, initialized: &mut BTreeSet<String>) -> bool {
        if occurrences(target, self.inner) != 0 {
            return false;
        }
        let field = self.field(target);
        let field_name = field.map(|f| ident(&f.name));
        if occurrences(value, self.inner) != 0 {
            if !field.is_some_and(private_final) || !self.creation(value, initialized) {
                return false;
            }
            self.storage.insert(field_name.clone().unwrap());
        }
        if let Some(name) = field_name {
            initialized.insert(name);
        }
        true
    }
    fn block(&mut self, block: &Block, initialized: &mut BTreeSet<String>) -> bool {
        for stmt in &block.statements {
            let valid = match stmt {
                Stmt::Assign { target, value } => self.assign(target, value, initialized),
                Stmt::Expr(Expr::Binary {
                    op: BinaryOp::Assign,
                    left,
                    right,
                }) => self.assign(left, right, initialized),
                Stmt::VarDecl { initializer, .. } => initializer
                    .as_ref()
                    .is_none_or(|e| occurrences(e, self.inner) == 0),
                Stmt::Expr(e) => occurrences(e, self.inner) == 0,
                Stmt::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    if occurrences(condition, self.inner) != 0 {
                        return false;
                    }
                    let mut yes = initialized.clone();
                    let mut no = initialized.clone();
                    if !self.block(then_block, &mut yes)
                        || else_block.as_ref().is_some_and(|b| !self.block(b, &mut no))
                    {
                        return false;
                    }
                    *initialized = yes.intersection(&no).cloned().collect();
                    true
                }
                Stmt::Block(block) => self.block(block, initialized),
                Stmt::ConstructorInvocation {
                    kind: ConstructorInvocationKind::Super,
                    args,
                    ..
                } => args.is_empty(),
                _ => false, // Delegation, abrupt exits and complex control flow need more proof.
            };
            if !valid {
                return false;
            }
        }
        true
    }
}

fn candidate(
    program: &Program,
    outer: &TypeDecl,
    inner: &TypeDecl,
    captures: &CaptureFacts,
) -> Option<OuterFieldCapturePlan> {
    if outer.kind != TypeKind::Class
        || !outer.type_params.is_empty()
        || outer.extends.is_some()
        || inner.kind != TypeKind::Class
        || !inner.type_params.is_empty()
        || inner.extends.is_some()
        || !inner.modifiers.contains(&Modifier::Private)
        || !inner.modifiers.contains(&Modifier::Final)
        || inner.modifiers.contains(&Modifier::Static)
        || !outer.static_init_blocks.is_empty()
        || captures.outer_this_type_for_type(&inner.name).is_none()
        || outer
            .members
            .iter()
            .any(|m| matches!(m, Member::Unsupported { .. }))
    {
        return None;
    }
    let fields = outer
        .members
        .iter()
        .filter_map(|m| {
            if let Member::Field(f) = m {
                Some(f)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let field_names = fields
        .iter()
        .map(|f| ident(&f.name))
        .collect::<HashSet<_>>();
    if fields.len() != field_names.len()
        || bound_names(inner, true)
            .iter()
            .any(|n| field_names.contains(n))
    {
        return None;
    }
    let own = inner
        .members
        .iter()
        .filter_map(|m| {
            if let Member::Field(f) = m {
                Some(ident(&f.name))
            } else {
                None
            }
        })
        .collect();
    let mut reads = Reads {
        fields: &fields,
        own,
        captured: BTreeSet::new(),
        captures,
    };
    let mut methods = HashSet::new();
    for member in &inner.members {
        match member {
            Member::Method(method) if !method.modifiers.contains(&Modifier::Native) => {
                if !reads.body(method.body.as_ref()?) {
                    return None;
                }
                methods.insert(ident(&method.name));
            }
            Member::Constructor(ctor) if ctor.type_params.is_empty() => {
                if !reads.body(ctor.body.as_ref()?) {
                    return None;
                }
            }
            Member::Field(field)
                if field
                    .initializer
                    .as_ref()
                    .is_none_or(|e| matches!(e, Expr::Literal(_))) => {}
            _ => return None,
        }
    }
    let mut init = Initialization {
        inner,
        fields: &fields,
        required: &reads.captured,
        storage: HashSet::new(),
        count: 0,
        bound: HashSet::new(),
    };
    let mut base = BTreeSet::new();
    for field in &fields {
        if static_field(field) {
            continue;
        }
        if let Some(value) = &field.initializer {
            if occurrences(value, inner) != 0 {
                if !private_final(field) || !init.creation(value, &base) {
                    return None;
                }
                init.storage.insert(ident(&field.name));
            }
            base.insert(ident(&field.name));
        }
    }
    for member in &outer.members {
        if let Member::Constructor(ctor) = member {
            let mut initialized = base.clone();
            init.bound = ctor.params.iter().map(|p| ident(&p.name)).collect();
            crate::semantic_analysis::walk_block_stmts(ctor.body.as_ref()?, &mut |s| {
                if let Stmt::VarDecl { name, .. } = s {
                    init.bound.insert(ident(name));
                }
            });
            if !init.block(ctor.body.as_ref()?, &mut initialized) {
                return None;
            }
        }
    }
    let mut total = 0;
    crate::semantic_analysis::walk_program_exprs(program, &mut |e| {
        total += usize::from(new_is(e, inner))
    });
    if total == 0
        || total != init.count
        || bound_names(outer, false)
            .iter()
            .any(|n| init.storage.contains(n))
    {
        return None;
    }
    // Check every expression root in the nest, not only the captured class.
    fn contained(ty: &TypeDecl, storage: &HashSet<String>, methods: &HashSet<String>) -> bool {
        if !ty.static_init_blocks.is_empty() {
            return false;
        }
        for member in &ty.members {
            let body = match member {
                Member::Type(nested) => {
                    if !contained(nested, storage, methods) {
                        return false;
                    }
                    continue;
                }
                Member::Field(f) => {
                    if f.initializer
                        .as_ref()
                        .is_some_and(|e| !storage_expr(e, storage, methods))
                    {
                        return false;
                    }
                    continue;
                }
                Member::Method(m) => m.body.as_ref(),
                Member::Constructor(c) => c.body.as_ref(),
                _ => return false,
            };
            let Some(body) = body else {
                return false;
            };
            let mut valid = true;
            crate::semantic_analysis::walk_block_stmts(body, &mut |s| {
                valid &= match s {
                    Stmt::Assign { target, value } => {
                        let initialization_target = matches!(target, Expr::Var(name) if storage.contains(&ident(name)))
                            || matches!(target, Expr::FieldAccess(receiver, name)
                                if matches!(receiver.as_ref(), Expr::This) && storage.contains(&ident(name)));
                        (initialization_target || storage_expr(target, storage, methods))
                            && storage_expr(value, storage, methods)
                    }
                    Stmt::Expr(e) | Stmt::Throw(e) => storage_expr(e, storage, methods),
                    Stmt::Return(e) | Stmt::VarDecl { initializer: e, .. } => {
                        e.as_ref().is_none_or(|e| storage_expr(e, storage, methods))
                    }
                    Stmt::If { condition, .. }
                    | Stmt::While { condition, .. }
                    | Stmt::DoWhile { condition, .. } => storage_expr(condition, storage, methods),
                    Stmt::For {
                        condition, update, ..
                    } => {
                        condition
                            .as_ref()
                            .is_none_or(|e| storage_expr(e, storage, methods))
                            && update.iter().all(|e| storage_expr(e, storage, methods))
                    }
                    Stmt::EnhancedFor { iterable, .. } => storage_expr(iterable, storage, methods),
                    Stmt::ConstructorInvocation { args, .. } => {
                        args.iter().all(|e| storage_expr(e, storage, methods))
                    }
                    Stmt::Switch { .. } | Stmt::LocalType(_) | Stmt::Unsupported(_) => false,
                    _ => true,
                };
            });
            if !valid {
                return false;
            }
        }
        true
    }
    if !contained(outer, &init.storage, &methods) {
        return None;
    }
    Some(OuterFieldCapturePlan {
        inner_type: inner.name.clone(),
        outer_type: outer.name.clone(),
        fields: fields
            .iter()
            .copied()
            .filter(|f| reads.captured.contains(&ident(&f.name)))
            .map(|field| OuterCapturedField {
                key: FieldKey::source_field(outer, field),
                rust_name: crate::java_naming::source_instance_field_rust_name(outer, &field.name),
                java_ty: field.ty.clone(),
            })
            .collect(),
    })
}

pub(crate) fn analyze(program: &Program, captures: &mut CaptureFacts) {
    // This flag is unioned across compilation units along with capture facts.
    // A partial source unit must not enable reduction while a different unit
    // uses observable GC/reflection/native lifetime mechanisms.
    captures.has_lifetime_observers |= program.imports.iter().any(|i| {
        [
            "java.lang.ref.",
            "java.lang.reflect.",
            "java.lang.invoke.",
            "sun.misc.Unsafe",
            "jdk.internal.misc.Unsafe",
        ]
        .iter()
        .any(|p| i.path.starts_with(p))
    });
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| {
        captures.has_lifetime_observers |= matches!(expr, Expr::Raw(_) | Expr::Unsupported { .. })
            || matches!(expr, Expr::Call { name, .. } if matches!(name.as_str(),
                "getDeclaredField" | "getDeclaredFields" | "getField" | "getFields" | "setAccessible"
                | "getDeclaredConstructor" | "getDeclaredConstructors" | "getUnsafe" | "findVarHandle"
                | "findGetter" | "findSetter" | "writeObject" | "getDeclaredMethod" | "getDeclaredMethods"
                | "getMethods" | "forName" | "newInstance" | "loadClass" | "defineClass"))
            || matches!(expr, Expr::New { ty: JavaType::Named { name, .. }, .. }
                if ["WeakReference", "SoftReference", "PhantomReference", "Cleaner", "Unsafe"].iter().any(|n| name.rsplit('.').next() == Some(*n)));
    });
    fn observer_name(name: &str) -> bool {
        ["java.lang.ref.", "java.lang.reflect.", "java.lang.invoke."]
            .iter()
            .any(|p| name.starts_with(p))
            || matches!(
                name.rsplit('.').next(),
                Some(
                    "WeakReference"
                        | "SoftReference"
                        | "PhantomReference"
                        | "Cleaner"
                        | "Unsafe"
                        | "ObjectOutputStream"
                        | "ObjectInputStream"
                )
            )
    }
    fn observer_type(ty: &JavaType) -> bool {
        match ty {
            JavaType::Named { name, args } => observer_name(name) || args.iter().any(observer_type),
            JavaType::Array(t) => observer_type(t),
            JavaType::Intersection(types) => types.iter().any(observer_type),
            JavaType::Wildcard(Some(
                crate::java_ir::WildcardBound::Extends(t) | crate::java_ir::WildcardBound::Super(t),
            )) => observer_type(t),
            JavaType::Unknown(name) => observer_name(name),
            _ => false,
        }
    }
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| {
        captures.has_lifetime_observers |= match expr {
            Expr::Cast { ty, .. }
            | Expr::InstanceOf { ty, .. }
            | Expr::New { ty, .. }
            | Expr::ClassLiteral(ty) => observer_type(ty),
            Expr::Call {
                resolved, receiver, ..
            } => {
                resolved.as_ref().is_some_and(|call| {
                    observer_name(&call.declaring_type) || observer_type(&call.return_type)
                }) || receiver.as_ref().is_some_and(|receiver| {
                    crate::stdlib::qualified_static_owner_java_name(receiver)
                        .is_some_and(|name| observer_name(&name))
                        || matches!(receiver.as_ref(), Expr::Var(name) if observer_name(name))
                })
            }
            _ => false,
        };
    });
    fn body_observers(body: &Block) -> bool {
        let mut found = false;
        crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
            found |= match stmt {
                Stmt::VarDecl { ty, .. } => observer_type(ty),
                Stmt::EnhancedFor { var_type, .. } => observer_type(var_type),
                Stmt::Try { catches, .. } => {
                    catches.iter().any(|c| c.types.iter().any(observer_type))
                }
                Stmt::LocalType(ty) => observers(ty),
                _ => false,
            };
        });
        found
    }
    fn observers(ty: &TypeDecl) -> bool {
        ty.extends.as_deref().is_some_and(observer_name)
            || ty.implements.iter().any(|n| observer_name(n))
            || ty.type_param_bounds.iter().flatten().any(observer_type)
            || ty
                .static_init_blocks
                .iter()
                .any(|b| body_observers(&b.block))
            || ty.members.iter().any(|m| match m {
                Member::Field(f) => observer_type(&f.ty),
                Member::Method(m) => {
                    m.name == "finalize"
                        || m.modifiers.contains(&Modifier::Native)
                        || observer_type(&m.return_type)
                        || m.params.iter().any(|p| observer_type(&p.ty))
                        || m.body.as_ref().is_some_and(body_observers)
                }
                Member::Constructor(c) => {
                    c.params.iter().any(|p| observer_type(&p.ty))
                        || c.body.as_ref().is_some_and(body_observers)
                }
                Member::Type(t) => observers(t),
                _ => false,
            })
    }
    captures.has_lifetime_observers |= program.types.iter().any(observers);
    if captures.has_lifetime_observers {
        return;
    }
    let mut plans = Vec::new();
    for outer in &program.types {
        for member in &outer.members {
            if let Member::Type(inner) = member {
                if let Some(plan) = candidate(program, outer, inner, captures) {
                    plans.push(plan);
                }
            }
        }
    }
    for plan in &plans {
        if let Some(owner) = captures
            .owners
            .iter_mut()
            .find(|o| o.name.as_deref() == Some(plan.inner_type.as_str()))
        {
            for field in &plan.fields {
                // Reuse reference-capture alias/storage propagation, rather
                // than cloning a source object payload into the environment.
                owner.captures.push(Capture {
                    binding: None,
                    name: field.key.field_name().into(),
                    source: CaptureSource::ThisField,
                    ty: field.java_ty.clone(),
                    effectively_final: true,
                    mutable_after_capture: false,
                });
            }
        }
    }
    captures.outer_field_plans.extend(plans);
}

/// All consumers use this same post-solve admission and physical tuple ABI.
/// Native payload containers and erased/value carriers do not qualify merely
/// because Rust can Clone them: a Java reference must copy a canonical handle.
pub(crate) fn selected<'a>(
    name: &str,
    facts: &'a FactSet,
    signatures: &SignaturePlan,
    representations: Option<&RepresentationPlan>,
) -> Option<(&'a OuterFieldCapturePlan, Vec<RustType>)> {
    let plan = facts.captures.outer_field_plan(name)?;
    if representations.is_some_and(|r| {
        matches!(
            r.representation_for(&JavaType::Named {
                name: plan.outer_type.clone(),
                args: vec![]
            }),
            crate::representation::ObjectRepresentation::SourceHierarchyCarrier { .. }
        )
    }) {
        return None;
    }
    let mut types = Vec::new();
    for field in &plan.fields {
        let shape = signatures.field_shape(field.key.type_name(), field.key.field_name())?;
        if !matches!(
            shape.ownership,
            OwnershipShape::Owned | OwnershipShape::CopyValue
        ) {
            return None;
        }
        let ty = if let Some(r) = representations {
            crate::slot_render::rust_type_for_slot_with_facts_and_representations(shape, facts, r)
        } else {
            crate::slot_render::rust_type_for_slot_with_facts(shape, facts)
        };
        let core = if let RustType::Option(inner) = &ty {
            inner.as_ref()
        } else {
            &ty
        };
        let primitive = matches!(field.java_ty, JavaType::Primitive(_))
            && matches!(
                core,
                RustType::Bool
                    | RustType::I8
                    | RustType::I16
                    | RustType::I32
                    | RustType::I64
                    | RustType::F32
                    | RustType::F64
                    | RustType::Char
            );
        let source_reference = crate::effective_facts::EffectiveFacts::new(facts)
            .parsed_source_controls_native_carrier(&field.java_ty)
            && matches!(core, RustType::Generic { base, args } if args.len() == 1
                && matches!(base.as_str(), "Rc" | "std::rc::Rc" | "Arc" | "std::sync::Arc"));
        if !primitive && !source_reference {
            return None;
        }
        types.push(ty);
    }
    Some((plan, types))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
        import java.util.function.IntSupplier;
        class Data { int value; int bump() { return ++value; } }
        final class Owner implements IntSupplier {
            private final Data payload;
            private final IntSupplier operation;
            Owner(Data payload) {
                this.payload = payload;
                operation = new Step();
            }
            private final class Step implements IntSupplier {
                public int getAsInt() { return payload.bump(); }
            }
            public int getAsInt() { return operation.getAsInt(); }
        }
    "#;

    fn facts(source: &str) -> CaptureFacts {
        let parsed = crate::parser::parse_java_source(source);
        crate::resolve::resolve_program(&parsed.program, parsed.diagnostics)
            .facts
            .captures
    }

    #[test]
    fn final_reference_read_uses_the_existing_capture_alias_contract() {
        let captures = facts(SOURCE);
        assert_eq!(captures.outer_field_plans.len(), 1, "{captures:#?}");
        let plan = &captures.outer_field_plans[0];
        assert_eq!(plan.fields.len(), 1);
        assert_eq!(plan.fields[0].key.field_name(), "payload");
        assert!(
            captures.owners.iter().any(|owner| owner
                .captures
                .iter()
                .any(|capture| capture.name == "payload"
                    && capture.source == CaptureSource::ThisField))
        );
    }

    #[test]
    fn incomplete_or_observable_environments_keep_the_outer_reference() {
        for (name, source) in [
            ("mutable field", SOURCE.replace("private final Data", "private Data")),
            ("early capture", SOURCE.replace(
                "this.payload = payload;\n                operation = new Step();",
                "operation = new Step(); this.payload = payload;")),
            ("escaping strategy", SOURCE.replace(
                "public int getAsInt() { return operation.getAsInt(); }",
                "public IntSupplier escape() { return operation; }")),
            ("publishing inner", SOURCE.replace(
                "public int getAsInt() { return payload.bump(); }",
                "public int getAsInt() { return payload.bump(); } Object escape() { return this; }")),
            ("outer method", SOURCE.replace("return payload.bump();", "return getAsInt();")),
            ("shadowed reference", SOURCE.replace("return payload.bump();", "Data payload = new Data(); return payload.bump();")),
            ("qualified outer identity", SOURCE.replace("return payload.bump();", "return Owner.this.getAsInt();")),
            ("unproved constructor control", SOURCE.replace("operation = new Step();", "while (false) {} operation = new Step();")),
            ("static initializer", SOURCE.replace("private final Data payload;", "static { int unused = 1; } private final Data payload;")),
        ] {
            let captures = facts(&source);
            assert!(captures.outer_field_plans.is_empty(), "{name}: {captures:#?}");
            assert!(captures.outer_this_type_for_type("Owner.Step").is_some(), "{name}: fallback lost");
        }
    }

    #[test]
    fn initialization_must_dominate_both_conditional_capture_paths() {
        let source = SOURCE
            .replace("Owner(Data payload)", "Owner(Data payload, boolean flip)")
            .replace(
                "this.payload = payload;",
                "if (flip) { this.payload = payload; } else { this.payload = payload; }",
            )
            .replace(
                "operation = new Step();",
                "operation = flip ? new Step() : new Step();",
            );
        assert_eq!(facts(&source).outer_field_plans.len(), 1);
        let incomplete = source.replace(
            "else { this.payload = payload; }",
            "else { int unused = 0; }",
        );
        assert!(facts(&incomplete).outer_field_plans.is_empty());
    }

    #[test]
    fn lifetime_observers_disable_plans_across_merged_source_units() {
        for observer in [
            "class Observer { java.lang.ref.WeakReference<Owner> owner; }",
            "class Observer { native void inspect(); }",
            "class Observer { protected void finalize() {} }",
            "class Observer { void inspect(java.io.ObjectOutputStream stream, Object value) throws java.io.IOException { stream.writeObject(value); } }",
            "class Observer { void inspect() { java.lang.ref.Reference.reachabilityFence(null); } }",
            "class Observer { void inspect() { java.lang.ref.WeakReference<Owner> ref = null; } }",
        ] {
            let mut merged = FactSet {
                captures: facts(SOURCE),
                ..FactSet::default()
            };
            assert_eq!(merged.captures.outer_field_plans.len(), 1);
            let observer = facts(observer);
            assert!(observer.has_lifetime_observers);
            merged.extend_from(FactSet {
                captures: observer,
                ..FactSet::default()
            });
            assert!(merged.captures.outer_field_plan("Owner.Step").is_none());
        }
    }

    #[test]
    fn serializable_payload_marker_is_not_itself_a_lifetime_observation() {
        let source = SOURCE.replace(
            "class Data {",
            "class Data implements java.io.Serializable {",
        );
        let captures = facts(&source);
        assert!(!captures.has_lifetime_observers);
        assert_eq!(captures.outer_field_plans.len(), 1);
    }
}
