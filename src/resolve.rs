use std::collections::{HashMap, HashSet};

use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    Capture, CaptureFacts, CaptureOrigin, CaptureOwner, CaptureOwnerKind, CaptureSource,
    ClosedHierarchyFact, FactProvenance, FactSet, NodeId, ObjectSafetyFact, ObjectSafetyViolation,
    TypeCapabilityOverride,
};
use crate::java_ir::{
    Block, CallKind, ConstructorDecl, ConstructorInvocationKind, Expr, JavaType, LambdaBody,
    Member, MethodDecl, Modifier, Param, Program, ResolvedCall, ResolvedConstructor, Stmt,
    TypeDecl, TypeKind,
};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
    pub facts: FactSet,
}

pub fn resolve_program(program: &Program, mut diagnostics: DiagnosticBag) -> ResolvedProgram {
    let mut program = program.clone();
    for ty in &mut program.types {
        assign_type_rust_names(ty, &mut diagnostics);
    }
    let symbols = ProgramSymbols::from_program(&program);
    for ty in &mut program.types {
        resolve_type_calls(ty, &symbols, &mut diagnostics);
    }
    let facts = collect_resolved_facts(&program);
    ResolvedProgram {
        program,
        diagnostics,
        facts,
    }
}

fn assign_type_rust_names(ty: &mut TypeDecl, diagnostics: &mut DiagnosticBag) {
    let mut method_counts: HashMap<String, usize> = HashMap::new();
    let mut ctor_count = 0usize;
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                *method_counts.entry(method.name.clone()).or_default() += 1;
            }
            Member::Constructor(_) => ctor_count += 1,
            _ => {}
        }
    }

    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => assign_type_rust_names(nested, diagnostics),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
}

fn resolve_type_calls(
    ty: &mut TypeDecl,
    symbols: &ProgramSymbols,
    diagnostics: &mut DiagnosticBag,
) {
    let methods = symbols.methods_for(&ty.name);
    let constructors = symbols.constructors_for(&ty.name);
    let fields = collect_field_env(ty);
    for member in &mut ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &mut method.body {
                    let mut env = fields.clone();
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        methods,
                        constructors,
                        &mut env,
                        diagnostics,
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &mut ctor.body {
                    let mut env = fields.clone();
                    for param in &ctor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        methods,
                        constructors,
                        &mut env,
                        diagnostics,
                    );
                }
            }
            Member::Type(nested) => resolve_type_calls(nested, symbols, diagnostics),
            _ => {}
        }
    }
}

fn resolve_method(method: &mut MethodDecl, counts: &HashMap<String, usize>) {
    let base = crate::parser::to_snake_case(&method.name);
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 {
        method.rust_name = Some(base);
        return;
    }

    let suffix = method
        .params
        .iter()
        .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
        .collect::<Vec<_>>()
        .join("_");
    method.rust_name = Some(format!("{base}_{suffix}"));
}

fn resolve_constructor(ctor: &mut ConstructorDecl, ctor_count: usize) {
    if ctor.params.is_empty() || ctor_count == 1 {
        ctor.rust_name = Some("new".to_string());
    } else {
        let suffix = ctor
            .params
            .iter()
            .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
            .collect::<Vec<_>>()
            .join("_");
        ctor.rust_name = Some(format!("new_{suffix}"));
    }
}

#[derive(Debug, Clone)]
struct MethodSymbol {
    declaring_type: String,
    java_name: String,
    rust_name: String,
    params: Vec<Param>,
    return_type: JavaType,
    is_static: bool,
}

#[derive(Debug, Clone)]
struct ConstructorSymbol {
    declaring_type: String,
    rust_name: String,
    params: Vec<Param>,
}

fn collect_method_symbols(ty: &TypeDecl) -> Vec<MethodSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(MethodSymbol {
                declaring_type: ty.name.clone(),
                java_name: method.name.clone(),
                rust_name: method
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
                is_static: method.modifiers.contains(&Modifier::Static),
            }),
            _ => None,
        })
        .collect()
}

fn collect_constructor_symbols(ty: &TypeDecl) -> Vec<ConstructorSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => Some(ConstructorSymbol {
                declaring_type: ty.name.clone(),
                rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                params: ctor.params.clone(),
            }),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct ProgramSymbols {
    methods_by_type: HashMap<String, Vec<MethodSymbol>>,
    constructors_by_type: HashMap<String, Vec<ConstructorSymbol>>,
}

impl ProgramSymbols {
    fn from_program(program: &Program) -> Self {
        let mut symbols = Self::default();
        for ty in &program.types {
            symbols.collect_type(ty);
        }
        symbols
    }

    fn collect_type(&mut self, ty: &TypeDecl) {
        self.methods_by_type
            .insert(ty.name.clone(), collect_method_symbols(ty));
        self.constructors_by_type
            .insert(ty.name.clone(), collect_constructor_symbols(ty));
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.collect_type(nested);
            }
        }
    }

    fn methods_for(&self, type_name: &str) -> &[MethodSymbol] {
        self.methods_by_type
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn constructors_for(&self, type_name: &str) -> &[ConstructorSymbol] {
        self.constructors_by_type
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

fn collect_field_env(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    env
}

fn resolve_block_calls(
    block: &mut Block,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    constructors: &[ConstructorSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    for stmt in &mut block.statements {
        resolve_stmt_calls(
            stmt,
            current_type,
            symbols,
            methods,
            constructors,
            env,
            diagnostics,
        );
        env.learn_stmt(stmt);
    }
}

fn resolve_stmt_calls(
    stmt: &mut Stmt,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    constructors: &[ConstructorSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics);
        }
        Stmt::VarDecl { .. } => {}
        Stmt::Assign { target, value } => {
            resolve_expr_calls(target, current_type, symbols, methods, env, diagnostics);
            resolve_expr_calls(value, current_type, symbols, methods, env, diagnostics);
        }
        Stmt::Expr(expr) => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Stmt::Return(Some(expr)) => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            resolve_expr_calls(condition, current_type, symbols, methods, env, diagnostics);
            resolve_block_calls(
                then_block,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
            );
            if let Some(block) = else_block {
                resolve_block_calls(
                    block,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut env.clone(),
                    diagnostics,
                );
            }
        }
        Stmt::While { condition, body } => {
            resolve_expr_calls(condition, current_type, symbols, methods, env, diagnostics);
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                resolve_stmt_calls(
                    stmt,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    env,
                    diagnostics,
                );
                env.learn_stmt(stmt);
            }
            if let Some(condition) = condition {
                resolve_expr_calls(condition, current_type, symbols, methods, env, diagnostics);
            }
            for expr in update {
                resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics);
            }
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
        } => {
            resolve_expr_calls(iterable, current_type, symbols, methods, env, diagnostics);
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut body_env,
                diagnostics,
            );
        }
        Stmt::Switch { selector, cases } => {
            resolve_expr_calls(selector, current_type, symbols, methods, env, diagnostics);
            for case in cases {
                for label in &mut case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics);
                    }
                }
                let mut case_env = env.clone();
                for stmt in &mut case.statements {
                    resolve_stmt_calls(
                        stmt,
                        current_type,
                        symbols,
                        methods,
                        constructors,
                        &mut case_env,
                        diagnostics,
                    );
                    case_env.learn_stmt(stmt);
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
                resolve_stmt_calls(
                    resource,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    env,
                    diagnostics,
                );
                env.learn_stmt(resource);
            }
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.name.clone(), ty.clone());
                }
                resolve_block_calls(
                    &mut catch.body,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut catch_env,
                    diagnostics,
                );
            }
            if let Some(block) = finally_block {
                resolve_block_calls(
                    block,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut env.clone(),
                    diagnostics,
                );
            }
        }
        Stmt::Throw(expr) => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            for arg in args.iter_mut() {
                resolve_expr_calls(arg, current_type, symbols, methods, env, diagnostics);
            }
            if matches!(kind, ConstructorInvocationKind::This) {
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                match resolve_same_class_constructor(&arg_types, constructors) {
                    Ok(Some(constructor)) => *resolved = Some(constructor),
                    Ok(None) => {}
                    Err(message) => diagnostics.error("resolve_constructor_ambiguous", message),
                }
            }
        }
        Stmt::Block(block) => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            constructors,
            &mut env.clone(),
            diagnostics,
        ),
        _ => {}
    }
}

fn resolve_expr_calls(
    expr: &mut Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            resolve_expr_calls(receiver, current_type, symbols, methods, env, diagnostics)
        }
        Expr::ArrayAccess(array, index) => {
            resolve_expr_calls(array, current_type, symbols, methods, env, diagnostics);
            resolve_expr_calls(index, current_type, symbols, methods, env, diagnostics);
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr_calls(left, current_type, symbols, methods, env, diagnostics);
            resolve_expr_calls(right, current_type, symbols, methods, env, diagnostics);
        }
        Expr::Unary { expr, .. } => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            resolve_expr_calls(condition, current_type, symbols, methods, env, diagnostics);
            resolve_expr_calls(then_expr, current_type, symbols, methods, env, diagnostics);
            resolve_expr_calls(else_expr, current_type, symbols, methods, env, diagnostics);
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            if let Some(receiver) = receiver {
                resolve_expr_calls(receiver, current_type, symbols, methods, env, diagnostics);
            }
            for arg in args.iter_mut() {
                resolve_expr_calls(arg, current_type, symbols, methods, env, diagnostics);
            }

            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                match resolve_same_class_call(name, &arg_types, methods) {
                    Ok(Some(call)) => *resolved = Some(call),
                    Ok(None) => {}
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some(receiver_ty) = receiver
                .as_deref()
                .map(|receiver| infer_expr_type(receiver, env))
                .and_then(|ty| match ty {
                    JavaType::Named { name, .. } | JavaType::Unknown(name)
                        if symbols.methods_by_type.contains_key(&name) =>
                    {
                        Some(name)
                    }
                    _ => None,
                })
            {
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                match resolve_same_class_call(name, &arg_types, symbols.methods_for(&receiver_ty)) {
                    Ok(Some(call)) => *resolved = Some(call),
                    Ok(None) => {}
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            }
        }
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => {
            for arg in args.iter_mut() {
                resolve_expr_calls(arg, current_type, symbols, methods, env, diagnostics);
            }
            if let JavaType::Named { name, .. } | JavaType::Unknown(name) = ty {
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                match resolve_same_class_constructor(&arg_types, symbols.constructors_for(name)) {
                    Ok(Some(constructor)) => *resolved = Some(constructor),
                    Ok(None) => {}
                    Err(message) => diagnostics.error("resolve_constructor_ambiguous", message),
                }
            }
            if let Some(members) = body {
                for member in members {
                    match member {
                        Member::Method(method) => {
                            if let Some(body) = &mut method.body {
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut env.clone(),
                                    diagnostics,
                                );
                            }
                        }
                        Member::Constructor(ctor) => {
                            if let Some(body) = &mut ctor.body {
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut env.clone(),
                                    diagnostics,
                                );
                            }
                        }
                        Member::Type(ty) => assign_type_rust_names(ty, diagnostics),
                        _ => {}
                    }
                }
            }
        }
        Expr::Cast { expr, .. } => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Expr::InstanceOf { expr, .. } => {
            resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                resolve_expr_calls(expr, current_type, symbols, methods, env, diagnostics)
            }
            LambdaBody::Block(block) => resolve_block_calls(
                block,
                current_type,
                symbols,
                methods,
                &[],
                &mut env.clone(),
                diagnostics,
            ),
        },
        _ => {}
    }
}

fn resolve_same_class_call(
    name: &str,
    arg_types: &[JavaType],
    methods: &[MethodSymbol],
) -> Result<Option<ResolvedCall>, String> {
    let candidates = methods
        .iter()
        .filter(|method| method.java_name == name && method.params.len() == arg_types.len())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|method| {
            method
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| arg_ty == &param.ty)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|method| {
            method
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| java_type_assignable(arg_ty, &param.ty))
        })
        .collect::<Vec<_>>();

    let selected = match exact.as_slice() {
        [one] => *one,
        [] if assignable.len() == 1 => assignable[0],
        [] if assignable.is_empty() && candidates.len() == 1 => candidates[0],
        [] => {
            return Err(format!(
                "no overload of {name} matches argument types {:?}",
                arg_types
            ));
        }
        _ => {
            return Err(format!(
                "ambiguous overload of {name} for argument types {:?}",
                arg_types
            ));
        }
    };

    Ok(Some(ResolvedCall {
        declaring_type: selected.declaring_type.clone(),
        rust_name: selected.rust_name.clone(),
        kind: if selected.is_static {
            CallKind::Static
        } else {
            CallKind::Instance
        },
        return_type: selected.return_type.clone(),
    }))
}

fn resolve_same_class_constructor(
    arg_types: &[JavaType],
    constructors: &[ConstructorSymbol],
) -> Result<Option<ResolvedConstructor>, String> {
    let candidates = constructors
        .iter()
        .filter(|constructor| constructor.params.len() == arg_types.len())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            constructor
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| arg_ty == &param.ty)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            constructor
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| java_type_assignable(arg_ty, &param.ty))
        })
        .collect::<Vec<_>>();

    let selected = match exact.as_slice() {
        [one] => *one,
        [] if assignable.len() == 1 => assignable[0],
        [] if assignable.is_empty() && candidates.len() == 1 => candidates[0],
        [] => {
            return Err(format!(
                "no constructor matches argument types {:?}",
                arg_types
            ));
        }
        _ => {
            return Err(format!(
                "ambiguous constructor for argument types {:?}",
                arg_types
            ));
        }
    };

    Ok(Some(ResolvedConstructor {
        declaring_type: selected.declaring_type.clone(),
        rust_name: selected.rust_name.clone(),
    }))
}

fn java_type_assignable(actual: &JavaType, target: &JavaType) -> bool {
    if actual == target {
        return true;
    }
    match (actual, target) {
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_rank(*actual) <= primitive_rank(*target)
        }
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => true,
        (JavaType::Unknown(name), _) if name == "null" => !matches!(target, JavaType::Primitive(_)),
        _ => false,
    }
}

fn primitive_rank(primitive: crate::java_ir::PrimitiveType) -> u8 {
    use crate::java_ir::PrimitiveType::*;
    match primitive {
        Boolean => 0,
        Byte => 1,
        Short | Char => 2,
        Int => 3,
        Long => 4,
        Float => 5,
        Double => 6,
    }
}

fn collect_resolved_facts(program: &Program) -> FactSet {
    let mut facts = collect_capture_facts(program);
    collect_type_capability_facts(program, &mut facts);
    facts
}

fn collect_capture_facts(program: &Program) -> FactSet {
    let mut analyzer = CaptureAnalyzer::default();
    for ty in &program.types {
        analyzer.analyze_top_level_type(ty);
    }
    FactSet {
        captures: CaptureFacts {
            owners: analyzer.owners,
        },
        ..FactSet::default()
    }
}

fn collect_type_capability_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_capability_facts_for_type(ty, facts);
    }
}

fn collect_type_capability_facts_for_type(ty: &TypeDecl, facts: &mut FactSet) {
    let object_safety = (ty.kind == TypeKind::Interface).then(|| interface_object_safety_fact(ty));
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: ty.name.clone(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: ty
                .implements
                .iter()
                .map(|name| JavaType::Named {
                    name: name.clone(),
                    args: Vec::new(),
                })
                .collect(),
            extends: ty.extends.as_ref().map(|name| JavaType::Named {
                name: name.clone(),
                args: Vec::new(),
            }),
            final_type: Some(ty.modifiers.contains(&Modifier::Final)),
            object_safe_trait: object_safety.as_ref().map(|fact| fact.object_safe),
            provenance: FactProvenance::ParsedSource,
        });
    if let Some(object_safety) = object_safety {
        facts.object_safety_facts.push(object_safety);
    }
    if ty.modifiers.contains(&Modifier::Sealed) && !ty.permits.is_empty() {
        facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
            type_name: ty.name.clone(),
            closed: true,
            provenance: FactProvenance::ParsedSource,
        });
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_capability_facts_for_type(nested, facts);
        }
    }
}

fn interface_object_safety_fact(ty: &TypeDecl) -> ObjectSafetyFact {
    let mut violations = Vec::new();
    collect_interface_object_safety_violations(ty, &mut violations);
    ObjectSafetyFact {
        type_name: ty.name.clone(),
        object_safe: violations.is_empty(),
        violations,
        provenance: FactProvenance::ParsedSource,
    }
}

fn collect_interface_object_safety_violations(
    ty: &TypeDecl,
    violations: &mut Vec<ObjectSafetyViolation>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                push_interface_method_object_safety_violations(&ty.name, method, violations)
            }
            Member::Type(nested) => collect_interface_object_safety_violations(nested, violations),
            Member::Unsupported { .. } => violations.push(ObjectSafetyViolation::UnsupportedMember),
            Member::Field(field) => violations.push(ObjectSafetyViolation::FieldMember {
                field_name: field.name.clone(),
            }),
            Member::Constructor(_) => violations.push(ObjectSafetyViolation::ConstructorMember),
        }
    }
}

fn push_interface_method_object_safety_violations(
    interface_name: &str,
    method: &MethodDecl,
    violations: &mut Vec<ObjectSafetyViolation>,
) {
    if method.modifiers.contains(&Modifier::Static) {
        violations.push(ObjectSafetyViolation::StaticMethod {
            method_name: method.name.clone(),
        });
    }
    match java_type_self_reference_kind(&method.return_type, interface_name) {
        Some(InterfaceSelfReferenceKind::Direct) => {
            violations.push(ObjectSafetyViolation::SelfReferentialReturn {
                method_name: method.name.clone(),
            });
        }
        Some(InterfaceSelfReferenceKind::Nested) => {
            violations.push(ObjectSafetyViolation::NestedSelfReferentialReturn {
                method_name: method.name.clone(),
            });
        }
        None => {}
    }
    for param in &method.params {
        match java_type_self_reference_kind(&param.ty, interface_name) {
            Some(InterfaceSelfReferenceKind::Direct) => {
                violations.push(ObjectSafetyViolation::SelfReferentialParameter {
                    method_name: method.name.clone(),
                    param_name: param.name.clone(),
                });
            }
            Some(InterfaceSelfReferenceKind::Nested) => {
                violations.push(ObjectSafetyViolation::NestedSelfReferentialParameter {
                    method_name: method.name.clone(),
                    param_name: param.name.clone(),
                });
            }
            None => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterfaceSelfReferenceKind {
    Direct,
    Nested,
}

fn java_type_self_reference_kind(
    java_ty: &JavaType,
    type_name: &str,
) -> Option<InterfaceSelfReferenceKind> {
    match java_ty {
        JavaType::Named { name, args } => {
            if name == type_name {
                Some(InterfaceSelfReferenceKind::Direct)
            } else if args
                .iter()
                .any(|arg| java_type_self_reference_kind(arg, type_name).is_some())
            {
                Some(InterfaceSelfReferenceKind::Nested)
            } else {
                None
            }
        }
        JavaType::Array(inner) => java_type_self_reference_kind(inner, type_name)
            .map(|_| InterfaceSelfReferenceKind::Nested),
        JavaType::Wildcard(Some(bound)) => match bound {
            crate::java_ir::WildcardBound::Extends(bound)
            | crate::java_ir::WildcardBound::Super(bound) => {
                java_type_self_reference_kind(bound, type_name)
                    .map(|_| InterfaceSelfReferenceKind::Nested)
            }
        },
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => None,
    }
}

#[derive(Debug, Default)]
struct CaptureAnalyzer {
    next_id: usize,
    owners: Vec<CaptureOwner>,
}

#[derive(Debug, Clone)]
struct ScopeFrame {
    symbols: HashMap<String, ScopeSymbol>,
    capturable: bool,
}

#[derive(Debug, Clone)]
struct ScopeSymbol {
    ty: JavaType,
    source: CaptureSource,
}

#[derive(Debug, Clone)]
struct CaptureContext {
    type_name: String,
    method_name: Option<String>,
    fields: HashMap<String, JavaType>,
    outer_type_name: Option<String>,
    outer_fields: HashMap<String, JavaType>,
    scopes: Vec<ScopeFrame>,
    mutated_names: HashSet<String>,
}

impl CaptureAnalyzer {
    fn analyze_top_level_type(&mut self, ty: &TypeDecl) {
        self.analyze_type_members(ty, None);
    }

    fn analyze_type_members(&mut self, ty: &TypeDecl, outer: Option<&TypeDecl>) {
        let fields = field_types(ty);
        let outer_fields = outer.map(field_types).unwrap_or_default();
        let outer_type_name = outer.map(|ty| ty.name.clone());

        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: Some(method.name.clone()),
                            fields: fields.clone(),
                            outer_type_name: outer_type_name.clone(),
                            outer_fields: outer_fields.clone(),
                            scopes: vec![method_param_frame(&method.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.analyze_block(body, &mut ctx, None);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: None,
                            fields: fields.clone(),
                            outer_type_name: outer_type_name.clone(),
                            outer_fields: outer_fields.clone(),
                            scopes: vec![method_param_frame(&ctor.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.analyze_block(body, &mut ctx, None);
                    }
                }
                Member::Type(inner) => self.analyze_inner_type(inner, ty),
                _ => {}
            }
        }
    }

    fn analyze_inner_type(&mut self, inner: &TypeDecl, outer: &TypeDecl) {
        let mut captures = Vec::new();
        self.analyze_type_members_for_owner(inner, Some(outer), &mut captures);
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::InnerClass,
            name: Some(inner.name.clone()),
            target_type: Some(JavaType::Named {
                name: inner.name.clone(),
                args: Vec::new(),
            }),
            origin: CaptureOrigin::Type {
                type_name: outer.name.clone(),
            },
            captures,
        });
        self.analyze_type_members(inner, Some(outer));
    }

    fn analyze_type_members_for_owner(
        &mut self,
        ty: &TypeDecl,
        outer: Option<&TypeDecl>,
        captures: &mut Vec<Capture>,
    ) {
        let fields = field_types(ty);
        let outer_fields = outer.map(field_types).unwrap_or_default();
        let outer_type_name = outer.map(|ty| ty.name.clone());

        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: Some(method.name.clone()),
                            fields: fields.clone(),
                            outer_type_name: outer_type_name.clone(),
                            outer_fields: outer_fields.clone(),
                            scopes: vec![method_param_frame(&method.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: None,
                            fields: fields.clone(),
                            outer_type_name: outer_type_name.clone(),
                            outer_fields: outer_fields.clone(),
                            scopes: vec![method_param_frame(&ctor.params, false)],
                            mutated_names: mutated_names_in_block(body),
                        };
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                _ => {}
            }
        }
    }

    fn analyze_block(
        &mut self,
        block: &Block,
        ctx: &mut CaptureContext,
        owner_captures: Option<&mut Vec<Capture>>,
    ) {
        let mut owner_captures = owner_captures;
        for stmt in &block.statements {
            match &mut owner_captures {
                Some(captures) => self.collect_stmt_captures(stmt, ctx, captures),
                None => self.analyze_stmt(stmt, ctx),
            }
            if let Stmt::VarDecl { ty, name, .. } = stmt {
                add_symbol(ctx, name, ty.clone(), CaptureSource::Local, false);
            }
        }
    }

    fn analyze_stmt(&mut self, stmt: &Stmt, ctx: &mut CaptureContext) {
        match stmt {
            Stmt::VarDecl {
                initializer: Some(expr),
                ..
            } => {
                self.analyze_expr(expr, ctx);
            }
            Stmt::VarDecl { .. } => {}
            Stmt::Assign { target, value } => {
                self.analyze_expr(target, ctx);
                self.analyze_expr(value, ctx);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                self.analyze_expr(expr, ctx);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_block(then_block, &mut ctx.clone(), None);
                if let Some(block) = else_block {
                    self.analyze_block(block, &mut ctx.clone(), None);
                }
            }
            Stmt::While { condition, body } => {
                self.analyze_expr(condition, ctx);
                self.analyze_block(body, &mut ctx.clone(), None);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let mut loop_ctx = ctx.clone();
                for stmt in init {
                    self.analyze_stmt(stmt, &mut loop_ctx);
                    if let Stmt::VarDecl { ty, name, .. } = stmt {
                        add_symbol(&mut loop_ctx, name, ty.clone(), CaptureSource::Local, false);
                    }
                }
                if let Some(expr) = condition {
                    self.analyze_expr(expr, &mut loop_ctx);
                }
                for expr in update {
                    self.analyze_expr(expr, &mut loop_ctx);
                }
                self.analyze_block(body, &mut loop_ctx, None);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                iterable,
                body,
            } => {
                self.analyze_expr(iterable, ctx);
                let mut body_ctx = ctx.clone();
                add_symbol(
                    &mut body_ctx,
                    var_name,
                    var_type.clone(),
                    CaptureSource::Local,
                    false,
                );
                self.analyze_block(body, &mut body_ctx, None);
            }
            Stmt::Switch { selector, cases } => {
                self.analyze_expr(selector, ctx);
                for case in cases {
                    for label in &case.labels {
                        if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                            self.analyze_expr(expr, ctx);
                        }
                    }
                    let block = Block {
                        statements: case.statements.clone(),
                    };
                    self.analyze_block(&block, &mut ctx.clone(), None);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                for resource in resources {
                    self.analyze_stmt(resource, ctx);
                }
                self.analyze_block(body, &mut ctx.clone(), None);
                for catch in catches {
                    let mut catch_ctx = ctx.clone();
                    let ty = catch
                        .types
                        .first()
                        .cloned()
                        .unwrap_or_else(|| JavaType::Unknown(catch.name.clone()));
                    add_symbol(&mut catch_ctx, &catch.name, ty, CaptureSource::Local, false);
                    self.analyze_block(&catch.body, &mut catch_ctx, None);
                }
                if let Some(block) = finally_block {
                    self.analyze_block(block, &mut ctx.clone(), None);
                }
            }
            Stmt::LocalType(local) => self.analyze_local_type(local, ctx),
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
            }
            Stmt::Block(block) => self.analyze_block(block, &mut ctx.clone(), None),
            _ => {}
        }
    }

    fn analyze_local_type(&mut self, local: &TypeDecl, ctx: &CaptureContext) {
        let mut captures = Vec::new();
        self.analyze_type_with_outer_scopes(local, ctx, &mut captures);
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::LocalClass,
            name: Some(local.name.clone()),
            target_type: Some(JavaType::Named {
                name: local.name.clone(),
                args: Vec::new(),
            }),
            origin: current_origin(ctx),
            captures,
        });
    }

    fn analyze_type_with_outer_scopes(
        &mut self,
        ty: &TypeDecl,
        outer_ctx: &CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        let fields = field_types(ty);
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: Some(method.name.clone()),
                            fields: fields.clone(),
                            outer_type_name: Some(outer_ctx.type_name.clone()),
                            outer_fields: outer_ctx.fields.clone(),
                            scopes: capturable_outer_scopes(outer_ctx),
                            mutated_names: outer_ctx.mutated_names.clone(),
                        };
                        ctx.scopes.push(method_param_frame(&method.params, false));
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                Member::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        let mut ctx = CaptureContext {
                            type_name: ty.name.clone(),
                            method_name: None,
                            fields: fields.clone(),
                            outer_type_name: Some(outer_ctx.type_name.clone()),
                            outer_fields: outer_ctx.fields.clone(),
                            scopes: capturable_outer_scopes(outer_ctx),
                            mutated_names: outer_ctx.mutated_names.clone(),
                        };
                        ctx.scopes.push(method_param_frame(&ctor.params, false));
                        self.collect_block_captures(body, &mut ctx, captures);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_block_captures(
        &mut self,
        block: &Block,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        self.analyze_block(block, ctx, Some(captures));
    }

    fn collect_stmt_captures(
        &mut self,
        stmt: &Stmt,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        match stmt {
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    self.collect_expr_captures(expr, ctx, captures);
                }
            }
            Stmt::Assign { target, value } => {
                self.collect_expr_captures(target, ctx, captures);
                self.collect_expr_captures(value, ctx, captures);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                self.collect_expr_captures(expr, ctx, captures);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                self.collect_expr_captures(condition, ctx, captures);
                self.collect_block_captures(then_block, &mut ctx.clone(), captures);
                if let Some(block) = else_block {
                    self.collect_block_captures(block, &mut ctx.clone(), captures);
                }
            }
            Stmt::Block(block) => self.collect_block_captures(block, &mut ctx.clone(), captures),
            _ => self.analyze_stmt(stmt, ctx),
        }
    }

    fn analyze_expr(&mut self, expr: &Expr, ctx: &mut CaptureContext) {
        match expr {
            Expr::Lambda { params, body } => self.analyze_lambda(params, body, ctx),
            Expr::New { args, body, ty, .. } => {
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
                if let Some(members) = body {
                    self.analyze_anonymous_class(ty, members, ctx);
                }
            }
            Expr::FieldAccess(receiver, _) => self.analyze_expr(receiver, ctx),
            Expr::ArrayAccess(array, index) => {
                self.analyze_expr(array, ctx);
                self.analyze_expr(index, ctx);
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left, ctx);
                self.analyze_expr(right, ctx);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.analyze_expr(expr, ctx);
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.analyze_expr(condition, ctx);
                self.analyze_expr(then_expr, ctx);
                self.analyze_expr(else_expr, ctx);
            }
            Expr::Call { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.analyze_expr(receiver, ctx);
                }
                for arg in args {
                    self.analyze_expr(arg, ctx);
                }
            }
            _ => {}
        }
    }

    fn analyze_lambda(
        &mut self,
        params: &[crate::java_ir::LambdaParam],
        body: &LambdaBody,
        ctx: &CaptureContext,
    ) {
        let mut lambda_ctx = ctx.clone();
        lambda_ctx.scopes = capturable_outer_scopes(ctx);
        lambda_ctx.scopes.push(lambda_param_frame(params, false));
        let mut captures = Vec::new();
        match body {
            LambdaBody::Expr(expr) => {
                self.collect_expr_captures(expr, &mut lambda_ctx, &mut captures);
            }
            LambdaBody::Block(block) => {
                self.collect_block_captures(block, &mut lambda_ctx, &mut captures);
            }
        }
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::Lambda,
            name: None,
            target_type: None,
            origin: current_origin(ctx),
            captures,
        });
    }

    fn analyze_anonymous_class(&mut self, ty: &JavaType, members: &[Member], ctx: &CaptureContext) {
        let synthetic = TypeDecl {
            kind: crate::java_ir::TypeKind::Class,
            name: "Anonymous".to_string(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: members.to_vec(),
            span: Default::default(),
        };
        let mut captures = Vec::new();
        self.analyze_type_with_outer_scopes(&synthetic, ctx, &mut captures);
        let id = self.next_node_id();
        self.push_owner(CaptureOwner {
            id,
            kind: CaptureOwnerKind::AnonymousClass,
            name: None,
            target_type: Some(ty.clone()),
            origin: current_origin(ctx),
            captures,
        });
    }

    fn collect_expr_captures(
        &mut self,
        expr: &Expr,
        ctx: &mut CaptureContext,
        captures: &mut Vec<Capture>,
    ) {
        match expr {
            Expr::Var(name) => capture_var(name, ctx, captures),
            Expr::Lambda { params, body } => self.analyze_lambda(params, body, ctx),
            Expr::New { ty, args, body, .. } => {
                for arg in args {
                    self.collect_expr_captures(arg, ctx, captures);
                }
                if let Some(members) = body {
                    self.analyze_anonymous_class(ty, members, ctx);
                }
            }
            Expr::FieldAccess(receiver, _) => self.collect_expr_captures(receiver, ctx, captures),
            Expr::ArrayAccess(array, index) => {
                self.collect_expr_captures(array, ctx, captures);
                self.collect_expr_captures(index, ctx, captures);
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr_captures(left, ctx, captures);
                self.collect_expr_captures(right, ctx, captures);
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.collect_expr_captures(expr, ctx, captures);
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_expr_captures(condition, ctx, captures);
                self.collect_expr_captures(then_expr, ctx, captures);
                self.collect_expr_captures(else_expr, ctx, captures);
            }
            Expr::Call { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.collect_expr_captures(receiver, ctx, captures);
                }
                for arg in args {
                    self.collect_expr_captures(arg, ctx, captures);
                }
            }
            _ => {}
        }
    }

    fn next_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    fn push_owner(&mut self, mut owner: CaptureOwner) {
        dedup_captures(&mut owner.captures);
        self.owners.push(owner);
    }
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

fn method_param_frame(params: &[Param], capturable: bool) -> ScopeFrame {
    ScopeFrame {
        symbols: params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    ScopeSymbol {
                        ty: param.ty.clone(),
                        source: CaptureSource::Parameter,
                    },
                )
            })
            .collect(),
        capturable,
    }
}

fn lambda_param_frame(params: &[crate::java_ir::LambdaParam], capturable: bool) -> ScopeFrame {
    ScopeFrame {
        symbols: params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    ScopeSymbol {
                        ty: param
                            .ty
                            .clone()
                            .unwrap_or_else(|| JavaType::Unknown(param.name.clone())),
                        source: CaptureSource::Parameter,
                    },
                )
            })
            .collect(),
        capturable,
    }
}

fn capturable_outer_scopes(ctx: &CaptureContext) -> Vec<ScopeFrame> {
    ctx.scopes
        .iter()
        .map(|frame| ScopeFrame {
            symbols: frame.symbols.clone(),
            capturable: true,
        })
        .collect()
}

fn add_symbol(
    ctx: &mut CaptureContext,
    name: &str,
    ty: JavaType,
    source: CaptureSource,
    capturable: bool,
) {
    if ctx.scopes.is_empty() {
        ctx.scopes.push(ScopeFrame {
            symbols: HashMap::new(),
            capturable,
        });
    }
    let idx = ctx.scopes.len() - 1;
    ctx.scopes[idx]
        .symbols
        .insert(name.to_string(), ScopeSymbol { ty, source });
}

fn capture_var(name: &str, ctx: &CaptureContext, captures: &mut Vec<Capture>) {
    for frame in ctx.scopes.iter().rev() {
        if let Some(symbol) = frame.symbols.get(name) {
            if frame.capturable {
                captures.push(Capture {
                    name: name.to_string(),
                    source: symbol.source,
                    ty: symbol.ty.clone(),
                    effectively_final: !ctx.mutated_names.contains(name),
                    mutable_after_capture: ctx.mutated_names.contains(name),
                });
            }
            return;
        }
    }

    if ctx.fields.contains_key(name) {
        return;
    }

    if let (Some(outer_name), Some(_)) = (ctx.outer_type_name.as_ref(), ctx.outer_fields.get(name))
    {
        captures.push(Capture {
            name: outer_name.clone(),
            source: CaptureSource::OuterThis,
            ty: JavaType::Named {
                name: outer_name.clone(),
                args: Vec::new(),
            },
            effectively_final: true,
            mutable_after_capture: false,
        });
    }
}

fn current_origin(ctx: &CaptureContext) -> CaptureOrigin {
    if let Some(method_name) = &ctx.method_name {
        CaptureOrigin::Method {
            type_name: ctx.type_name.clone(),
            method_name: method_name.clone(),
        }
    } else {
        CaptureOrigin::Constructor {
            type_name: ctx.type_name.clone(),
        }
    }
}

fn dedup_captures(captures: &mut Vec<Capture>) {
    let mut seen = HashSet::new();
    captures.retain(|capture| seen.insert((capture.name.clone(), capture.source)));
}

fn mutated_names_in_block(block: &Block) -> HashSet<String> {
    let mut out = HashSet::new();
    for stmt in &block.statements {
        collect_mutated_names_stmt(stmt, &mut out);
    }
    out
}

fn collect_mutated_names_stmt(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign { target, value } => {
            collect_assignment_target_name(target, out);
            collect_mutated_names_expr(value, out);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_mutated_names_expr(expr, out);
        }
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => {
            collect_mutated_names_expr(expr, out);
        }
        Stmt::VarDecl { .. } => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_mutated_names_expr(condition, out);
            for stmt in &then_block.statements {
                collect_mutated_names_stmt(stmt, out);
            }
            if let Some(block) = else_block {
                for stmt in &block.statements {
                    collect_mutated_names_stmt(stmt, out);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_mutated_names_expr(condition, out);
            for stmt in &body.statements {
                collect_mutated_names_stmt(stmt, out);
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
                collect_mutated_names_stmt(stmt, out);
            }
            if let Some(expr) = condition {
                collect_mutated_names_expr(expr, out);
            }
            for expr in update {
                collect_mutated_names_expr(expr, out);
            }
            for stmt in &body.statements {
                collect_mutated_names_stmt(stmt, out);
            }
        }
        Stmt::Block(block) => {
            for stmt in &block.statements {
                collect_mutated_names_stmt(stmt, out);
            }
        }
        _ => {}
    }
}

fn collect_mutated_names_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op:
                crate::java_ir::UnaryOp::PreInc
                | crate::java_ir::UnaryOp::PreDec
                | crate::java_ir::UnaryOp::PostInc
                | crate::java_ir::UnaryOp::PostDec,
            expr,
        } => collect_assignment_target_name(expr, out),
        Expr::FieldAccess(receiver, _) => collect_mutated_names_expr(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_mutated_names_expr(array, out);
            collect_mutated_names_expr(index, out);
        }
        Expr::Binary { left, right, .. } => {
            collect_mutated_names_expr(left, out);
            collect_mutated_names_expr(right, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutated_names_expr(condition, out);
            collect_mutated_names_expr(then_expr, out);
            collect_mutated_names_expr(else_expr, out);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutated_names_expr(receiver, out);
            }
            for arg in args {
                collect_mutated_names_expr(arg, out);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_mutated_names_expr(arg, out);
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_mutated_names_expr(expr, out);
        }
        _ => {}
    }
}

fn collect_assignment_target_name(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::FieldAccess(receiver, _) | Expr::ArrayAccess(receiver, _) => {
            collect_assignment_target_name(receiver, out);
        }
        _ => {}
    }
}
