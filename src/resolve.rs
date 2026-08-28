use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::binding_id::{BindingId, BindingKind};
use crate::diagnostics::DiagnosticBag;
use crate::facts::{
    Capture, CaptureFacts, CaptureOrigin, CaptureOwner, CaptureOwnerKind, CaptureSource,
    ClosedHierarchyFact, ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet, FieldKey,
    MethodEffectFact, MethodKey, NodeId, NullabilityState, ObjectSafetyFact, ObjectSafetyViolation,
    ReceiverMutabilityFact, SealedPermitsFact, TypeCapabilityOverride, UnresolvedTargetFact,
    UnresolvedTargetKind,
};
use crate::java_ir::{
    Block, CallKind, ConstructorDecl, ConstructorInvocationKind, Expr, JavaCoercionEvidence,
    JavaCoercionKind, JavaType, LambdaBody, Member, MethodDecl, Modifier, Param, PrimitiveType,
    Program, ResolvedCall, ResolvedConstructor, Stmt, SwitchLabel, TypeDecl, TypeKind,
    WildcardBound,
};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
    pub facts: FactSet,
}

#[derive(Debug, Clone)]
pub struct SourceSymbols {
    symbols: ProgramSymbols,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverloadApplicabilityMode {
    FixedArity,
    VariableArity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadArgumentConversion {
    Exact,
    PrimitiveWidening,
    Boxing,
    Unboxing,
    BoxingWidening,
    ReferenceWidening,
    GenericInference,
    NullToReference,
    LambdaToFunctionalInterface,
    MethodReferenceToFunctionalInterface,
    UnknownFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverloadRejectionReason {
    ArityMismatch {
        expected: usize,
        actual: usize,
        varargs: bool,
    },
    MissingVarargsArrayParam,
    ArgumentTypeMismatch {
        arg_index: usize,
        actual: JavaType,
        target: JavaType,
    },
    NullToPrimitive {
        arg_index: usize,
        target: JavaType,
    },
    LambdaTargetNotFunctionalInterface {
        arg_index: usize,
        target: JavaType,
    },
    MethodReferenceTargetNotFunctionalInterface {
        arg_index: usize,
        target: JavaType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadArgumentApplicability {
    pub arg_index: usize,
    pub actual: JavaType,
    pub target: JavaType,
    pub conversion: OverloadArgumentConversion,
    pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadApplicabilityReport {
    pub mode: Option<OverloadApplicabilityMode>,
    pub score: Option<u32>,
    pub arguments: Vec<OverloadArgumentApplicability>,
    pub rejections: Vec<OverloadRejectionReason>,
}

impl OverloadApplicabilityReport {
    pub fn is_applicable(&self) -> bool {
        self.score.is_some() && self.rejections.is_empty()
    }
}

pub fn resolve_program(program: &Program, diagnostics: DiagnosticBag) -> ResolvedProgram {
    resolve_program_with_source_program(program, diagnostics, program)
}

pub fn resolve_program_with_source_program(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_program: &Program,
) -> ResolvedProgram {
    let source_symbols = source_symbols_from_program(source_program);
    resolve_program_with_source_symbols(program, diagnostics, &source_symbols)
}

pub fn resolve_program_with_source_program_and_facts(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_program: &Program,
    external_facts: &FactSet,
) -> ResolvedProgram {
    let source_symbols = source_symbols_from_program(source_program);
    resolve_program_with_source_symbols_and_facts(
        program,
        diagnostics,
        &source_symbols,
        external_facts,
    )
}

pub fn source_symbols_from_program(source_program: &Program) -> SourceSymbols {
    let mut symbol_program = source_program.clone();
    let mut diagnostics = DiagnosticBag::new();
    qualify_nested_type_names(&mut symbol_program);
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &symbol_program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    for ty in &mut symbol_program.types {
        assign_type_rust_names(ty, &mut diagnostics, &program_counts);
    }
    SourceSymbols {
        symbols: ProgramSymbols::from_program(&symbol_program),
    }
}

/// Give every method in `program` its overload-disambiguated Rust name.
///
/// The primary program gets this during resolution; **context and summary
/// programs did not**, so a `MethodDecl` reaching lowering from one of them had
/// `rust_name: None` and `source_method_rust_name` fell back to the bare
/// snake-cased Java name. Two overloads then became one Rust method:
///
/// ```text
/// Util::pick_int(1), Util::pick_string("ab")   // static path:   resolved
/// u.inst(1),         u.inst("ab")              // instance path: collapsed
/// ```
///
/// The static path escaped because it re-solves signatures on an isolated
/// one-type program, which does name them. Two answers to one question, which is
/// what [0009](../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)
/// is about — and the naming pass was already correct, it was simply not applied
/// here.
///
/// Deliberately does **not** call `qualify_nested_type_names`: these programs
/// are looked up by the names they already carry, and requalifying them would
/// change lookups rather than names.
pub fn assign_rust_names_to_context_program(program: &mut Program) {
    let mut diagnostics = DiagnosticBag::new();
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    for ty in &mut program.types {
        assign_type_rust_names(ty, &mut diagnostics, &program_counts);
    }
}

pub fn resolve_program_with_source_symbols(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_symbols: &SourceSymbols,
) -> ResolvedProgram {
    resolve_program_with_source_symbols_and_facts(
        program,
        diagnostics,
        source_symbols,
        &FactSet::default(),
    )
}

pub fn resolve_program_with_source_symbols_and_facts(
    program: &Program,
    diagnostics: DiagnosticBag,
    source_symbols: &SourceSymbols,
    external_facts: &FactSet,
) -> ResolvedProgram {
    let mut diagnostics = diagnostics;
    let mut program = program.clone();
    qualify_nested_type_names(&mut program);
    assign_local_binding_ids(&mut program);
    let mut program_counts = ProgramMethodCounts::new();
    for ty in &program.types {
        collect_program_method_counts(ty, &mut program_counts);
    }
    let symbols = &source_symbols.symbols;
    for ty in &mut program.types {
        assign_type_rust_names_with_source_symbols(ty, &mut diagnostics, &program_counts, symbols);
    }
    let mut resolver_facts = FactSet::default();
    for ty in &mut program.types {
        resolve_type_calls(
            ty,
            symbols,
            &mut diagnostics,
            &mut resolver_facts,
            external_facts,
        );
    }
    let mut facts = collect_resolved_facts(&program);
    facts.extend_from(resolver_facts);
    ResolvedProgram {
        program,
        diagnostics,
        facts,
    }
}

pub fn declaration_facts_from_program(program: &Program) -> FactSet {
    let mut program = program.clone();
    qualify_nested_type_names(&mut program);
    let mut facts = FactSet::default();
    collect_type_capability_facts(&program, &mut facts);
    collect_field_declaration_facts(&program, &mut facts);
    collect_static_final_constant_value_facts(&program, &mut facts);
    collect_method_declaration_effect_facts(&program, &mut facts);
    facts
}

fn assign_local_binding_ids(program: &mut Program) {
    for ty in &mut program.types {
        assign_local_binding_ids_for_type(ty);
    }
}

fn assign_local_binding_ids_for_type(ty: &mut TypeDecl) {
    for member in &mut ty.members {
        match member {
            Member::Method(method) => {
                let mut occurrences = HashMap::new();
                for param in &mut method.params {
                    param.binding = Some(next_binding_id(
                        BindingKind::Param,
                        &param.source_name,
                        &mut occurrences,
                    ));
                }
                if let Some(body) = &mut method.body {
                    assign_local_binding_ids_for_block(body, &mut occurrences);
                }
            }
            Member::Constructor(constructor) => {
                let mut occurrences = HashMap::new();
                for param in &mut constructor.params {
                    param.binding = Some(next_binding_id(
                        BindingKind::Param,
                        &param.source_name,
                        &mut occurrences,
                    ));
                }
                if let Some(body) = &mut constructor.body {
                    assign_local_binding_ids_for_block(body, &mut occurrences);
                }
            }
            Member::Type(nested) => assign_local_binding_ids_for_type(nested),
            _ => {}
        }
    }
}

fn assign_local_binding_ids_for_block(
    block: &mut Block,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) {
    for stmt in &mut block.statements {
        assign_local_binding_ids_for_stmt(stmt, occurrences);
    }
}

fn assign_local_binding_ids_for_stmt(
    stmt: &mut Stmt,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) {
    match stmt {
        Stmt::VarDecl {
            source_name,
            binding,
            ..
        } => {
            *binding = Some(next_binding_id(
                BindingKind::Local,
                source_name,
                occurrences,
            ));
        }
        Stmt::For { init, body, .. } => {
            for init_stmt in init {
                assign_local_binding_ids_for_stmt(init_stmt, occurrences);
            }
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::EnhancedFor {
            source_var_name,
            body,
            binding,
            ..
        } => {
            *binding = Some(next_binding_id(
                BindingKind::LoopVariable,
                source_var_name,
                occurrences,
            ));
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            assign_local_binding_ids_for_block(then_block, occurrences);
            if let Some(else_block) = else_block {
                assign_local_binding_ids_for_block(else_block, occurrences);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            assign_local_binding_ids_for_block(body, occurrences);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                for stmt in &mut case.statements {
                    assign_local_binding_ids_for_stmt(stmt, occurrences);
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
                assign_local_binding_ids_for_stmt(resource, occurrences);
            }
            assign_local_binding_ids_for_block(body, occurrences);
            for catch in catches {
                catch.binding = Some(next_binding_id(
                    BindingKind::CatchParam,
                    &catch.source_name,
                    occurrences,
                ));
                assign_local_binding_ids_for_block(&mut catch.body, occurrences);
            }
            if let Some(finally_block) = finally_block {
                assign_local_binding_ids_for_block(finally_block, occurrences);
            }
        }
        Stmt::LocalType(local) => assign_local_binding_ids_for_type(local),
        Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn next_binding_id(
    kind: BindingKind,
    name: &str,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) -> BindingId {
    let key = (kind, name.to_string());
    let occurrence = occurrences.entry(key).or_default();
    let binding = BindingId::new(kind, name, *occurrence);
    *occurrence += 1;
    binding
}

fn collect_method_declaration_effect_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_type_method_declaration_effect_facts(ty, facts);
    }
}

fn collect_type_method_declaration_effect_facts(ty: &TypeDecl, facts: &mut FactSet) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let return_nullability = source_summary_method_return_nullability(method);
                if return_nullability != NullabilityState::Unknown || !method.throws.is_empty() {
                    facts.method_effect_facts.push(MethodEffectFact {
                        method: MethodKey {
                            type_name: ty.name.clone(),
                            method_name: method.name.clone(),
                        },
                        receiver_mutability: ReceiverMutabilityFact::Unknown,
                        return_nullability,
                        thrown: method.throws.clone(),
                        provenance: FactProvenance::ParsedSource,
                    });
                }
            }
            Member::Type(nested) => collect_type_method_declaration_effect_facts(nested, facts),
            _ => {}
        }
    }
}

fn source_summary_method_return_nullability(method: &MethodDecl) -> NullabilityState {
    crate::signature::source_method_return_nullability_constraints(method)
        .into_iter()
        .find_map(|constraint| match constraint {
            crate::signature::SignatureConstraint::RequiredStorageNullability(state) => Some(state),
            _ => None,
        })
        .unwrap_or_else(|| crate::semantic_analysis::method_return_nullability_effect(method))
}

fn qualify_nested_type_names(program: &mut Program) {
    for ty in &mut program.types {
        qualify_nested_type_names_for_type(ty, &HashMap::new());
    }
}

fn qualify_nested_type_names_for_type(
    ty: &mut TypeDecl,
    enclosing_nested: &HashMap<String, String>,
) {
    let mut direct_nested = HashMap::new();
    for member in &ty.members {
        if let Member::Type(nested) = member {
            let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
            let nested_qualified = format!("{}.{}", ty.name, nested_simple);
            direct_nested.insert(nested.name.clone(), nested_qualified.clone());
            collect_nested_type_path_aliases(
                nested,
                nested_simple.to_string(),
                nested_qualified,
                &mut direct_nested,
            );
        }
    }

    let mut visible_nested = enclosing_nested.clone();
    visible_nested.extend(direct_nested.clone());

    rewrite_type_decl_references(ty, &visible_nested);

    for member in &mut ty.members {
        if let Member::Type(nested) = member {
            if let Some(qualified) = direct_nested.get(&nested.name).cloned() {
                nested.name = qualified;
            }
            qualify_nested_type_names_for_type(nested, &visible_nested);
        }
    }
}

fn collect_nested_type_path_aliases(
    ty: &TypeDecl,
    reference_prefix: String,
    qualified_prefix: String,
    aliases: &mut HashMap<String, String>,
) {
    for member in &ty.members {
        if let Member::Type(nested) = member {
            let nested_simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
            let reference_name = format!("{reference_prefix}.{nested_simple}");
            let qualified_name = format!("{qualified_prefix}.{nested_simple}");
            aliases.insert(reference_name.clone(), qualified_name.clone());
            collect_nested_type_path_aliases(nested, reference_name, qualified_name, aliases);
        }
    }
}

fn rewrite_type_decl_references(ty: &mut TypeDecl, direct_nested: &HashMap<String, String>) {
    if let Some(extends) = &mut ty.extends
        && let Some(qualified) = direct_nested.get(extends)
    {
        *extends = qualified.clone();
    }
    for implements in &mut ty.implements {
        if let Some(qualified) = direct_nested.get(implements) {
            *implements = qualified.clone();
        }
    }
    for bound in &mut ty.type_param_bounds {
        if let Some(bound) = bound {
            qualify_nested_java_type(bound, direct_nested);
        }
    }
    for member in &mut ty.members {
        match member {
            Member::Field(field) => {
                qualify_nested_java_type(&mut field.ty, direct_nested);
                if let Some(initializer) = &mut field.initializer {
                    qualify_nested_expr_types(initializer, direct_nested);
                }
            }
            Member::Method(method) => {
                qualify_nested_java_type(&mut method.return_type, direct_nested);
                for bound in &mut method.type_param_bounds {
                    if let Some(bound) = bound {
                        qualify_nested_java_type(bound, direct_nested);
                    }
                }
                for param in &mut method.params {
                    qualify_nested_java_type(&mut param.ty, direct_nested);
                }
                for thrown in &mut method.throws {
                    qualify_nested_java_type(thrown, direct_nested);
                }
                if let Some(body) = &mut method.body {
                    qualify_nested_block_types(body, direct_nested);
                }
            }
            Member::Constructor(ctor) => {
                for bound in &mut ctor.type_param_bounds {
                    if let Some(bound) = bound {
                        qualify_nested_java_type(bound, direct_nested);
                    }
                }
                for param in &mut ctor.params {
                    qualify_nested_java_type(&mut param.ty, direct_nested);
                }
                for thrown in &mut ctor.throws {
                    qualify_nested_java_type(thrown, direct_nested);
                }
                if let Some(body) = &mut ctor.body {
                    qualify_nested_block_types(body, direct_nested);
                }
            }
            Member::Type(_) | Member::Unsupported { .. } => {}
        }
    }
}

fn qualify_nested_block_types(block: &mut Block, direct_nested: &HashMap<String, String>) {
    for stmt in &mut block.statements {
        qualify_nested_stmt_types(stmt, direct_nested);
    }
}

fn qualify_nested_stmt_types(stmt: &mut Stmt, direct_nested: &HashMap<String, String>) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            qualify_nested_java_type(ty, direct_nested);
            if let Some(initializer) = initializer {
                qualify_nested_expr_types(initializer, direct_nested);
            }
        }
        Stmt::Assign { target, value } => {
            qualify_nested_expr_types(target, direct_nested);
            qualify_nested_expr_types(value, direct_nested);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            qualify_nested_expr_types(expr, direct_nested);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            qualify_nested_expr_types(condition, direct_nested);
            qualify_nested_block_types(then_block, direct_nested);
            if let Some(else_block) = else_block {
                qualify_nested_block_types(else_block, direct_nested);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            qualify_nested_expr_types(condition, direct_nested);
            qualify_nested_block_types(body, direct_nested);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                qualify_nested_stmt_types(stmt, direct_nested);
            }
            if let Some(condition) = condition {
                qualify_nested_expr_types(condition, direct_nested);
            }
            for expr in update {
                qualify_nested_expr_types(expr, direct_nested);
            }
            qualify_nested_block_types(body, direct_nested);
        }
        Stmt::EnhancedFor {
            var_type,
            iterable,
            body,
            ..
        } => {
            qualify_nested_java_type(var_type, direct_nested);
            qualify_nested_expr_types(iterable, direct_nested);
            qualify_nested_block_types(body, direct_nested);
        }
        Stmt::Switch { selector, cases } => {
            qualify_nested_expr_types(selector, direct_nested);
            for case in cases {
                for stmt in &mut case.statements {
                    qualify_nested_stmt_types(stmt, direct_nested);
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
                qualify_nested_stmt_types(resource, direct_nested);
            }
            qualify_nested_block_types(body, direct_nested);
            for catch in catches {
                for ty in &mut catch.types {
                    qualify_nested_java_type(ty, direct_nested);
                }
                qualify_nested_block_types(&mut catch.body, direct_nested);
            }
            if let Some(finally_block) = finally_block {
                qualify_nested_block_types(finally_block, direct_nested);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested);
            }
        }
        Stmt::LocalType(local) => rewrite_type_decl_references(local, direct_nested),
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            qualify_nested_block_types(body, direct_nested)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn qualify_nested_expr_types(expr: &mut Expr, direct_nested: &HashMap<String, String>) {
    match expr {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            qualify_nested_expr_types(receiver, direct_nested);
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            qualify_nested_expr_types(array, direct_nested);
            qualify_nested_expr_types(index, direct_nested);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            qualify_nested_java_type(element_type, direct_nested);
            for expr in dimensions {
                qualify_nested_expr_types(expr, direct_nested);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    qualify_nested_expr_types(expr, direct_nested);
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for expr in values {
                qualify_nested_expr_types(expr, direct_nested);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            qualify_nested_expr_types(condition, direct_nested);
            qualify_nested_expr_types(then_expr, direct_nested);
            qualify_nested_expr_types(else_expr, direct_nested);
        }
        Expr::Switch { selector, cases } => {
            qualify_nested_expr_types(selector, direct_nested);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_nested_expr_types(expr, direct_nested);
                    }
                }
                qualify_nested_expr_types(&mut case.value, direct_nested);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                qualify_nested_expr_types(receiver, direct_nested);
            }
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested);
            }
        }
        Expr::New { ty, args, body, .. } => {
            qualify_nested_java_type(ty, direct_nested);
            for arg in args {
                qualify_nested_expr_types(arg, direct_nested);
            }
            if let Some(members) = body {
                for member in members {
                    match member {
                        Member::Field(field) => {
                            qualify_nested_java_type(&mut field.ty, direct_nested);
                            if let Some(initializer) = &mut field.initializer {
                                qualify_nested_expr_types(initializer, direct_nested);
                            }
                        }
                        Member::Method(method) => {
                            qualify_nested_java_type(&mut method.return_type, direct_nested);
                            for param in &mut method.params {
                                qualify_nested_java_type(&mut param.ty, direct_nested);
                            }
                            if let Some(body) = &mut method.body {
                                qualify_nested_block_types(body, direct_nested);
                            }
                        }
                        Member::Constructor(ctor) => {
                            for param in &mut ctor.params {
                                qualify_nested_java_type(&mut param.ty, direct_nested);
                            }
                            if let Some(body) = &mut ctor.body {
                                qualify_nested_block_types(body, direct_nested);
                            }
                        }
                        Member::Type(_) | Member::Unsupported { .. } => {}
                    }
                }
            }
        }
        Expr::Cast { ty, expr } | Expr::InstanceOf { ty, expr, .. } => {
            qualify_nested_java_type(ty, direct_nested);
            qualify_nested_expr_types(expr, direct_nested);
        }
        Expr::Lambda { params, body } => {
            for param in params {
                if let Some(ty) = &mut param.ty {
                    qualify_nested_java_type(ty, direct_nested);
                }
            }
            match body {
                LambdaBody::Expr(expr) => qualify_nested_expr_types(expr, direct_nested),
                LambdaBody::Block(block) => qualify_nested_block_types(block, direct_nested),
            }
        }
        Expr::ClassLiteral(ty) => qualify_nested_java_type(ty, direct_nested),
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn qualify_nested_java_type(ty: &mut JavaType, direct_nested: &HashMap<String, String>) {
    match ty {
        JavaType::Named { name, args } => {
            if let Some(qualified) = direct_nested.get(name) {
                *name = qualified.clone();
            }
            for arg in args {
                qualify_nested_java_type(arg, direct_nested);
            }
        }
        JavaType::Unknown(name) => {
            if let Some(qualified) = direct_nested.get(name) {
                *name = qualified.clone();
            }
        }
        JavaType::Array(inner) => qualify_nested_java_type(inner, direct_nested),
        JavaType::Wildcard(Some(bound)) => match bound {
            WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                qualify_nested_java_type(inner, direct_nested)
            }
        },
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {}
    }
}

/// Method-name counts per type, used to decide whether a name needs an overload
/// suffix. Counting only a type's *declared* methods gave a subclass that
/// overrides one of several overloads a different Rust name from the superclass
/// it overrides — same Java method, two names, which breaks the moment dispatch
/// is virtual.
type ProgramMethodCounts = HashMap<String, HashMap<String, usize>>;

fn collect_program_method_counts(ty: &TypeDecl, out: &mut ProgramMethodCounts) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for member in &ty.members {
        if let Member::Method(method) = member {
            *counts.entry(method.name.clone()).or_default() += 1;
        }
    }
    // Index under both names: nested types are stored qualified (`Outer.Inner`)
    // while an `extends` clause usually names them simply (`Inner`).
    let simple = ty.name.rsplit('.').next().unwrap_or(&ty.name).to_string();
    for key in [ty.name.clone(), simple] {
        let entry = out.entry(key).or_default();
        for (name, count) in &counts {
            let slot = entry.entry(name.clone()).or_default();
            *slot = (*slot).max(*count);
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_program_method_counts(nested, out);
        }
    }
}

/// Counts for `ty` including everything it inherits, so an override is named the
/// same way as the method it overrides.
fn method_counts_including_supertypes(
    ty: &TypeDecl,
    program_counts: &ProgramMethodCounts,
) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for member in &ty.members {
        if let Member::Method(method) = member {
            *counts.entry(method.name.clone()).or_default() += 1;
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut pending = ty.extends.clone().into_iter().collect::<Vec<_>>();
    while let Some(super_name) = pending.pop() {
        let simple = super_name
            .rsplit('.')
            .next()
            .unwrap_or(&super_name)
            .to_string();
        if !seen.insert(simple.clone()) {
            continue;
        }
        for key in [super_name.as_str(), simple.as_str()] {
            if let Some(super_counts) = program_counts.get(key) {
                for (name, count) in super_counts {
                    let entry = counts.entry(name.clone()).or_default();
                    *entry = (*entry).max(*count);
                }
            }
        }
    }
    counts
}

fn assign_type_rust_names(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
) {
    let method_counts = method_counts_including_supertypes(ty, program_counts);
    let mut ctor_count = 0usize;
    for member in &ty.members {
        if let Member::Constructor(_) = member {
            ctor_count += 1;
        }
    }
    assign_type_rust_names_with_counts(ty, diagnostics, program_counts, method_counts, ctor_count)
}

fn assign_type_rust_names_with_source_symbols(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
) {
    let method_counts = method_counts_including_source_hierarchy(ty, program_counts, symbols);
    let mut ctor_count = 0usize;
    for member in &ty.members {
        if let Member::Constructor(_) = member {
            ctor_count += 1;
        }
    }
    assign_type_rust_names_with_source_symbols_and_counts(
        ty,
        diagnostics,
        program_counts,
        symbols,
        method_counts,
        ctor_count,
    )
}

fn method_counts_including_source_hierarchy(
    ty: &TypeDecl,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for method in symbols.methods_for_hierarchy(&ty.name) {
        *counts.entry(method.java_name).or_default() += 1;
    }
    if counts.is_empty() {
        method_counts_including_supertypes(ty, program_counts)
    } else {
        counts
    }
}

fn assign_type_rust_names_with_counts(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    method_counts: HashMap<String, usize>,
    ctor_count: usize,
) {
    let extends = ty.extends.clone();
    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts, extends.as_deref()),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => assign_type_rust_names(nested, diagnostics, program_counts),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    allocate_unique_declared_method_rust_names(ty);
}

fn assign_type_rust_names_with_source_symbols_and_counts(
    ty: &mut TypeDecl,
    diagnostics: &mut DiagnosticBag,
    program_counts: &ProgramMethodCounts,
    symbols: &ProgramSymbols,
    method_counts: HashMap<String, usize>,
    ctor_count: usize,
) {
    let extends = ty.extends.clone();
    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts, extends.as_deref()),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => assign_type_rust_names_with_source_symbols(
                nested,
                diagnostics,
                program_counts,
                symbols,
            ),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    allocate_unique_declared_method_rust_names(ty);
}

fn resolve_type_calls(
    ty: &mut TypeDecl,
    symbols: &ProgramSymbols,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
) {
    let methods = symbols.methods_for_hierarchy(&ty.name);
    let constructors = symbols.constructors_for(&ty.name);
    let fields = collect_field_env(ty, symbols);
    for member in &mut ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &mut method.body {
                    let mut env = fields.clone();
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ty.type_params,
                        &ty.type_param_bounds,
                    );
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &method.type_params,
                        &method.type_param_bounds,
                    );
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        &methods,
                        constructors,
                        &mut env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some(&method.name),
                    );
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &mut ctor.body {
                    let mut env = fields.clone();
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ty.type_params,
                        &ty.type_param_bounds,
                    );
                    insert_scoped_type_param_bounds(
                        &mut env,
                        &ctor.type_params,
                        &ctor.type_param_bounds,
                    );
                    for param in &ctor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(
                        body,
                        &ty.name,
                        symbols,
                        &methods,
                        constructors,
                        &mut env,
                        diagnostics,
                        facts,
                        external_facts,
                        Some("<init>"),
                    );
                }
            }
            Member::Type(nested) => {
                resolve_type_calls(nested, symbols, diagnostics, facts, external_facts)
            }
            _ => {}
        }
    }
}

fn resolve_method(
    method: &mut MethodDecl,
    counts: &HashMap<String, usize>,
    direct_superclass: Option<&str>,
) {
    let base = crate::parser::to_snake_case(&method.name);
    if source_method_overrides_jdk_input_stream_read_3(method, direct_superclass) {
        method.rust_name = Some("read_3".to_string());
        return;
    }
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 {
        method.rust_name = Some(base);
        return;
    }

    // The zero-argument overload keeps the bare name. Only one overload of a
    // name can take no arguments, so this cannot collide, and the alternative
    // is a trailing underscore (`to_string_encoding_`) from joining an empty
    // suffix.
    if method.params.is_empty() {
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

fn allocate_unique_declared_method_rust_names(ty: &mut TypeDecl) {
    let mut occupied = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => ctor.rust_name.clone(),
            _ => None,
        })
        .collect::<HashSet<_>>();

    for member in &mut ty.members {
        let Member::Method(method) = member else {
            continue;
        };
        let base = method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
        let rust_name = crate::java_naming::allocate_rust_ident(&base, &occupied);
        occupied.insert(rust_name.clone());
        method.rust_name = Some(rust_name);
    }
}

fn source_method_overrides_jdk_input_stream_read_3(
    method: &MethodDecl,
    direct_superclass: Option<&str>,
) -> bool {
    method.name == "read"
        && method.params.len() == 3
        && direct_superclass
            .and_then(|name| name.rsplit('.').next())
            .is_some_and(|name| {
                matches!(
                    name,
                    "InputStream"
                        | "FileInputStream"
                        | "ByteArrayInputStream"
                        | "BufferedInputStream"
                        | "DataInputStream"
                        | "GZIPInputStream"
                        | "ZipInputStream"
                        | "ObjectInputStream"
                        | "FilterInputStream"
                        | "PushbackInputStream"
                        | "SequenceInputStream"
                )
            })
        && method.params.first().is_some_and(|param| {
            matches!(
                &param.ty,
                JavaType::Array(element)
                    if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
            )
        })
        && method
            .params
            .get(1)
            .is_some_and(|param| matches!(param.ty, JavaType::Primitive(PrimitiveType::Int)))
        && method
            .params
            .get(2)
            .is_some_and(|param| matches!(param.ty, JavaType::Primitive(PrimitiveType::Int)))
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

fn insert_scoped_type_param_bounds(
    env: &mut TypeEnv,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
) {
    for (name, bound) in type_params.iter().zip(type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct MethodSymbol {
    declaring_type: String,
    owner_type_params: Vec<String>,
    owner_type_param_bounds: Vec<Option<JavaType>>,
    method_type_params: Vec<String>,
    method_type_param_bounds: Vec<Option<JavaType>>,
    java_name: String,
    rust_name: String,
    params: Vec<Param>,
    return_type: JavaType,
    is_static: bool,
}

#[derive(Debug, Clone)]
struct MethodResolution {
    call: ResolvedCall,
    params: Vec<Param>,
}

#[derive(Debug, Clone)]
struct ConstructorSymbol {
    declaring_type: String,
    outer_type: Option<String>,
    rust_name: String,
    params: Vec<Param>,
}

#[derive(Debug, Clone)]
struct FieldSymbol {
    declaring_type: String,
    outer_type: Option<String>,
    java_name: String,
    ty: JavaType,
    is_static: bool,
}

fn collect_method_symbols(ty: &TypeDecl) -> Vec<MethodSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(MethodSymbol {
                declaring_type: ty.name.clone(),
                owner_type_params: ty.type_params.clone(),
                owner_type_param_bounds: ty.type_param_bounds.clone(),
                method_type_params: method.type_params.clone(),
                method_type_param_bounds: method.type_param_bounds.clone(),
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

fn collect_constructor_symbols(ty: &TypeDecl, outer: Option<&TypeDecl>) -> Vec<ConstructorSymbol> {
    let mut constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) => Some(ConstructorSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                params: ctor.params.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if constructors.is_empty() && matches!(ty.kind, TypeKind::Class | TypeKind::Record) {
        constructors.push(ConstructorSymbol {
            declaring_type: ty.name.clone(),
            outer_type: outer.map(|outer| outer.name.clone()),
            rust_name: "new".to_string(),
            params: Vec::new(),
        });
    }
    constructors
}

fn collect_field_symbols(ty: &TypeDecl, outer: Option<&TypeDecl>) -> Vec<FieldSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(FieldSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                java_name: field.name.clone(),
                ty: field.ty.clone(),
                is_static: field.modifiers.contains(&Modifier::Static),
            }),
            Member::Type(nested) => Some(FieldSymbol {
                declaring_type: ty.name.clone(),
                outer_type: outer.map(|outer| outer.name.clone()),
                java_name: crate::parser::to_snake_case(nested_simple_type_name(&nested.name)),
                ty: JavaType::Named {
                    name: nested.name.clone(),
                    args: Vec::new(),
                },
                is_static: true,
            }),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct ProgramSymbols {
    methods_by_type: HashMap<String, Vec<MethodSymbol>>,
    constructors_by_type: HashMap<String, Vec<ConstructorSymbol>>,
    fields_by_type: HashMap<String, Vec<FieldSymbol>>,
    hierarchy_methods_by_type: HashMap<String, Vec<MethodSymbol>>,
    hierarchy_fields_by_type: HashMap<String, Vec<FieldSymbol>>,
    source_hierarchy_walk_by_type: Arc<Mutex<HashMap<String, Vec<(String, u32)>>>>,
    extends_by_type: HashMap<String, String>,
    extends_type_by_type: HashMap<String, JavaType>,
    implements_by_type: HashMap<String, Vec<String>>,
    implements_type_by_type: HashMap<String, Vec<JavaType>>,
    outer_by_type: HashMap<String, String>,
    static_by_type: HashMap<String, bool>,
    canonical_type_names: HashMap<String, String>,
    kind_by_type: HashMap<String, TypeKind>,
    type_params_by_type: HashMap<String, Vec<String>>,
    type_param_bounds_by_type: HashMap<String, Vec<Option<JavaType>>>,
    functional_interfaces: HashSet<String>,
    functional_interface_methods: HashMap<String, MethodSymbol>,
}

impl ProgramSymbols {
    fn from_program(program: &Program) -> Self {
        let mut symbols = Self::default();
        for ty in &program.types {
            symbols.collect_type(ty, None);
        }
        symbols.rebuild_hierarchy_indexes();
        symbols
    }

    fn rebuild_hierarchy_indexes(&mut self) {
        self.hierarchy_methods_by_type.clear();
        self.hierarchy_fields_by_type.clear();
        let type_names = self
            .canonical_type_names
            .values()
            .cloned()
            .collect::<HashSet<_>>();
        for type_name in type_names {
            self.hierarchy_methods_by_type.insert(
                type_name.clone(),
                self.compute_methods_for_hierarchy(&type_name),
            );
            self.hierarchy_fields_by_type.insert(
                type_name.clone(),
                self.compute_fields_for_hierarchy(&type_name),
            );
        }
    }

    fn collect_type(&mut self, ty: &TypeDecl, outer: Option<&TypeDecl>) {
        let methods = collect_method_symbols(ty);
        self.methods_by_type
            .insert(ty.name.clone(), methods.clone());
        self.constructors_by_type
            .entry(ty.name.clone())
            .or_default()
            .extend(collect_constructor_symbols(ty, outer));
        self.fields_by_type
            .entry(ty.name.clone())
            .or_default()
            .extend(collect_field_symbols(ty, outer));
        self.kind_by_type.insert(ty.name.clone(), ty.kind);
        self.type_params_by_type
            .insert(ty.name.clone(), ty.type_params.clone());
        self.type_param_bounds_by_type
            .insert(ty.name.clone(), ty.type_param_bounds.clone());
        self.static_by_type
            .insert(ty.name.clone(), ty.modifiers.contains(&Modifier::Static));
        self.canonical_type_names
            .insert(ty.name.clone(), ty.name.clone());
        if let Some(outer) = outer {
            self.outer_by_type
                .insert(ty.name.clone(), outer.name.clone());
            let outer_aliases = self
                .canonical_type_names
                .iter()
                .filter_map(|(alias, source_name)| {
                    (source_name == &outer.name).then(|| alias.clone())
                })
                .collect::<Vec<_>>();
            for outer_alias in outer_aliases {
                self.canonical_type_names.insert(
                    format!("{outer_alias}.{}", nested_simple_type_name(&ty.name)),
                    ty.name.clone(),
                );
            }
        }
        if is_source_functional_interface(ty) {
            self.functional_interfaces.insert(ty.name.clone());
            if let Some(method) = methods.iter().find(|method| !method.is_static).cloned() {
                self.functional_interface_methods
                    .insert(ty.name.clone(), method);
            }
        }
        self.canonical_type_names
            .insert(crate::parser::to_snake_case(&ty.name), ty.name.clone());
        if let Some(super_type) = &ty.extends {
            let parsed_super_type = crate::parser::parse_type(super_type);
            self.extends_by_type
                .insert(ty.name.clone(), java_type_simple_name(&parsed_super_type));
            self.extends_type_by_type
                .insert(ty.name.clone(), parsed_super_type);
        }
        if !ty.implements.is_empty() {
            let implements_types = ty
                .implements
                .iter()
                .map(|interface| crate::parser::parse_type(interface))
                .collect::<Vec<_>>();
            self.implements_by_type.insert(
                ty.name.clone(),
                implements_types.iter().map(java_type_simple_name).collect(),
            );
            self.implements_type_by_type
                .insert(ty.name.clone(), implements_types);
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                self.collect_type(nested, Some(ty));
            }
        }
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        self.collect_local_types_from_block(body, ty);
                    }
                }
                Member::Constructor(constructor) => {
                    if let Some(body) = &constructor.body {
                        self.collect_local_types_from_block(body, ty);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_local_types_from_block(&mut self, block: &Block, enclosing_type: &TypeDecl) {
        for stmt in &block.statements {
            self.collect_local_types_from_stmt(stmt, enclosing_type);
        }
    }

    fn collect_local_types_from_stmt(&mut self, stmt: &Stmt, enclosing_type: &TypeDecl) {
        match stmt {
            Stmt::LocalType(local) => self.collect_type(local, Some(enclosing_type)),
            Stmt::VarDecl {
                initializer: Some(initializer),
                ..
            } => self.collect_local_types_from_expr(initializer, enclosing_type),
            Stmt::Assign { target, value } => {
                self.collect_local_types_from_expr(target, enclosing_type);
                self.collect_local_types_from_expr(value, enclosing_type);
            }
            Stmt::Expr(expr) | Stmt::Throw(expr) => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Stmt::Return(Some(expr)) => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_block(then_block, enclosing_type);
                if let Some(else_block) = else_block {
                    self.collect_local_types_from_block(else_block, enclosing_type);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                for stmt in init {
                    self.collect_local_types_from_stmt(stmt, enclosing_type);
                }
                if let Some(condition) = condition {
                    self.collect_local_types_from_expr(condition, enclosing_type);
                }
                for expr in update {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                self.collect_local_types_from_expr(iterable, enclosing_type);
                self.collect_local_types_from_block(body, enclosing_type);
            }
            Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
                self.collect_local_types_from_block(body, enclosing_type)
            }
            Stmt::Switch {
                selector, cases, ..
            } => {
                self.collect_local_types_from_expr(selector, enclosing_type);
                for case in cases {
                    for stmt in &case.statements {
                        self.collect_local_types_from_stmt(stmt, enclosing_type);
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
                    self.collect_local_types_from_stmt(resource, enclosing_type);
                }
                self.collect_local_types_from_block(body, enclosing_type);
                for catch in catches {
                    self.collect_local_types_from_block(&catch.body, enclosing_type);
                }
                if let Some(finally_block) = finally_block {
                    self.collect_local_types_from_block(finally_block, enclosing_type);
                }
            }
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
            }
            _ => {}
        }
    }

    fn collect_local_types_from_expr(&mut self, expr: &Expr, enclosing_type: &TypeDecl) {
        match expr {
            Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
                self.collect_local_types_from_expr(receiver, enclosing_type);
            }
            Expr::ArrayAccess(array, index)
            | Expr::Binary {
                left: array,
                right: index,
                ..
            } => {
                self.collect_local_types_from_expr(array, enclosing_type);
                self.collect_local_types_from_expr(index, enclosing_type);
            }
            Expr::ArrayCreation {
                dimensions,
                initializer,
                ..
            } => {
                for expr in dimensions {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
                if let Some(initializer) = initializer {
                    for expr in initializer {
                        self.collect_local_types_from_expr(expr, enclosing_type);
                    }
                }
            }
            Expr::ArrayInitializer(dimensions) => {
                for expr in dimensions {
                    self.collect_local_types_from_expr(expr, enclosing_type);
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.collect_local_types_from_expr(condition, enclosing_type);
                self.collect_local_types_from_expr(then_expr, enclosing_type);
                self.collect_local_types_from_expr(else_expr, enclosing_type);
            }
            Expr::Switch { selector, cases } => {
                self.collect_local_types_from_expr(selector, enclosing_type);
                for case in cases {
                    for label in &case.labels {
                        if let SwitchLabel::Expr(expr) = label {
                            self.collect_local_types_from_expr(expr, enclosing_type);
                        }
                    }
                    self.collect_local_types_from_expr(&case.value, enclosing_type);
                }
            }
            Expr::Call { receiver, args, .. } => {
                if let Some(receiver) = receiver {
                    self.collect_local_types_from_expr(receiver, enclosing_type);
                }
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
            }
            Expr::New { args, body, .. } => {
                for arg in args {
                    self.collect_local_types_from_expr(arg, enclosing_type);
                }
                if let Some(members) = body {
                    self.collect_local_types_from_members(members, enclosing_type);
                }
            }
            Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
                self.collect_local_types_from_expr(expr, enclosing_type);
            }
            Expr::Lambda { body, .. } => match body {
                LambdaBody::Expr(expr) => self.collect_local_types_from_expr(expr, enclosing_type),
                LambdaBody::Block(block) => {
                    self.collect_local_types_from_block(block, enclosing_type)
                }
            },
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

    fn collect_local_types_from_members(&mut self, members: &[Member], enclosing_type: &TypeDecl) {
        for member in members {
            match member {
                Member::Type(local) => self.collect_type(local, Some(enclosing_type)),
                Member::Method(method) => {
                    if let Some(body) = &method.body {
                        self.collect_local_types_from_block(body, enclosing_type);
                    }
                }
                Member::Constructor(constructor) => {
                    if let Some(body) = &constructor.body {
                        self.collect_local_types_from_block(body, enclosing_type);
                    }
                }
                _ => {}
            }
        }
    }

    fn methods_for_hierarchy(&self, type_name: &str) -> Vec<MethodSymbol> {
        let source_name = self.source_type_name(type_name);
        if let Some(methods) = self.hierarchy_methods_by_type.get(source_name) {
            return methods.clone();
        }
        self.compute_methods_for_hierarchy(source_name)
    }

    fn compute_methods_for_hierarchy(&self, type_name: &str) -> Vec<MethodSymbol> {
        let mut out = Vec::new();
        let mut seen_signatures = HashSet::new();
        let mut visited = HashSet::new();
        self.collect_methods_for_hierarchy(
            self.source_type_name(type_name),
            &HashMap::new(),
            &mut visited,
            &mut seen_signatures,
            &mut out,
        );
        out
    }

    fn collect_methods_for_hierarchy(
        &self,
        type_name: &str,
        substitutions: &HashMap<String, JavaType>,
        visited: &mut HashSet<String>,
        seen_signatures: &mut HashSet<String>,
        out: &mut Vec<MethodSymbol>,
    ) {
        let type_name = self.source_type_name(type_name).to_string();
        let visit_key = format!("{type_name}::{substitutions:?}");
        if !visited.insert(visit_key) {
            return;
        }
        if let Some(methods) = self.methods_by_type.get(&type_name) {
            for method in methods {
                let substituted = substitute_method_symbol(method, substitutions);
                if seen_signatures.insert(method_signature_key(&substituted)) {
                    out.push(substituted);
                }
            }
        }

        if let Some(super_type) = self.extends_type_by_type.get(&type_name) {
            let mut super_substitutions = substitutions.clone();
            if let Some(super_name) = extend_supertype_substitutions(
                self,
                &type_name,
                super_type,
                &mut super_substitutions,
            ) {
                self.collect_methods_for_hierarchy(
                    &super_name,
                    &super_substitutions,
                    visited,
                    seen_signatures,
                    out,
                );
            }
        }

        if let Some(interfaces) = self.implements_type_by_type.get(&type_name) {
            for interface_type in interfaces {
                let mut interface_substitutions = substitutions.clone();
                if let Some(interface_name) = extend_supertype_substitutions(
                    self,
                    &type_name,
                    interface_type,
                    &mut interface_substitutions,
                ) {
                    self.collect_methods_for_hierarchy(
                        &interface_name,
                        &interface_substitutions,
                        visited,
                        seen_signatures,
                        out,
                    );
                }
            }
        }
    }

    fn methods_for_unqualified_call(&self, type_name: &str) -> Vec<MethodSymbol> {
        let mut out = Vec::new();
        let mut seen_signatures = HashSet::new();
        let current_type_is_static = self.type_is_static(type_name);
        for (index, lexical_type) in self.lexical_type_scope(type_name).into_iter().enumerate() {
            for method in self.methods_for_hierarchy(&lexical_type) {
                if index > 0 && current_type_is_static && !method.is_static {
                    continue;
                }
                if seen_signatures.insert(method_signature_key(&method)) {
                    out.push(method);
                }
            }
        }
        out
    }

    fn type_is_static(&self, type_name: &str) -> bool {
        self.static_by_type
            .get(self.source_type_name(type_name))
            .copied()
            .unwrap_or(false)
    }

    fn constructors_for(&self, type_name: &str) -> &[ConstructorSymbol] {
        let type_name = self.source_type_name(type_name);
        self.constructors_by_type
            .get(type_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn constructors_for_new(&self, type_name: &str, current_type: &str) -> Vec<ConstructorSymbol> {
        let constructors = self.constructors_for(type_name);
        let scoped = constructors
            .iter()
            .filter(|constructor| self.constructor_visible_from(constructor, current_type))
            .cloned()
            .collect::<Vec<_>>();
        if scoped.is_empty() {
            constructors.to_vec()
        } else {
            scoped
        }
    }

    fn constructor_visible_from(
        &self,
        constructor: &ConstructorSymbol,
        current_type: &str,
    ) -> bool {
        let Some(outer_type) = constructor.outer_type.as_deref() else {
            return false;
        };
        self.lexical_type_scope(current_type)
            .iter()
            .any(|type_name| type_name == outer_type)
    }

    fn lexical_type_scope(&self, type_name: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            let source_name = self.source_type_name(&type_name).to_string();
            if !visited.insert(source_name.clone()) {
                break;
            }
            out.push(source_name.clone());
            current = self.outer_by_type.get(&source_name).cloned();
        }
        out
    }

    fn field_for_from(
        &self,
        type_name: &str,
        field_name: &str,
        current_type: &str,
    ) -> Option<&FieldSymbol> {
        let canonical_field_name = crate::parser::to_snake_case(field_name);
        let mut current = Some(self.field_lookup_start_type(type_name, current_type));
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                let matches = fields
                    .iter()
                    .filter(|field| field_name_matches(field, field_name, &canonical_field_name))
                    .collect::<Vec<_>>();
                if let Some(field) = self.preferred_scoped_field(matches, current_type) {
                    return Some(field);
                }
            }
            current = self.extends_by_type.get(type_name).cloned();
        }
        None
    }

    fn external_super_field_type_for(&self, type_name: &str, field_name: &str) -> Option<JavaType> {
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            let Some(super_ty) = self.extends_type_by_type.get(type_name) else {
                return None;
            };
            let super_name = java_type_simple_name(super_ty);
            if self.fields_by_type.contains_key(&super_name)
                || self.methods_by_type.contains_key(&super_name)
            {
                current = Some(super_name);
                continue;
            }
            if let Some(java_ty) = known_external_or_pack_field_type_for_receiver(
                super_ty,
                field_name,
                &FactSet::default(),
            ) {
                return Some(java_ty);
            }
            return None;
        }
        None
    }

    fn field_lookup_start_type(&self, type_name: &str, current_type: &str) -> String {
        let source_name = self.source_type_name(type_name);
        if self.fields_by_type.contains_key(source_name)
            || self.methods_by_type.contains_key(source_name)
        {
            return source_name.to_string();
        }
        self.type_param_bound_for(current_type, type_name)
            .as_ref()
            .map(java_type_simple_name)
            .unwrap_or_else(|| source_name.to_string())
    }

    fn static_field_for_from(
        &self,
        type_name: &str,
        field_name: &str,
        current_type: &str,
    ) -> Option<&FieldSymbol> {
        let canonical_field_name = crate::parser::to_snake_case(field_name);
        let mut current = Some(self.source_type_name(type_name));
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            if !visited.insert(type_name.to_string()) {
                return None;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                let matches = fields
                    .iter()
                    .filter(|field| {
                        field.is_static
                            && field_name_matches(field, field_name, &canonical_field_name)
                    })
                    .collect::<Vec<_>>();
                if let Some(field) = self.preferred_scoped_field(matches, current_type) {
                    return Some(field);
                }
            }
            current = self.extends_by_type.get(type_name).map(String::as_str);
        }
        None
    }

    fn preferred_scoped_field<'a>(
        &self,
        fields: Vec<&'a FieldSymbol>,
        current_type: &str,
    ) -> Option<&'a FieldSymbol> {
        if fields.len() <= 1 {
            return fields.into_iter().next();
        }
        let scoped = fields
            .iter()
            .copied()
            .filter(|field| self.field_visible_from(field, current_type))
            .collect::<Vec<_>>();
        match scoped.as_slice() {
            [field] => Some(*field),
            [] => fields.into_iter().next(),
            _ => scoped.into_iter().next(),
        }
    }

    fn field_visible_from(&self, field: &FieldSymbol, current_type: &str) -> bool {
        let Some(outer_type) = field.outer_type.as_deref() else {
            return false;
        };
        self.lexical_type_scope(current_type)
            .iter()
            .any(|type_name| type_name == outer_type)
    }

    fn fields_for_hierarchy(&self, type_name: &str) -> Vec<FieldSymbol> {
        let source_name = self.source_type_name(type_name);
        if let Some(fields) = self.hierarchy_fields_by_type.get(source_name) {
            return fields.clone();
        }
        self.compute_fields_for_hierarchy(source_name)
    }

    fn compute_fields_for_hierarchy(&self, type_name: &str) -> Vec<FieldSymbol> {
        let mut out = Vec::new();
        let mut seen_fields = HashSet::new();
        let mut current = Some(self.source_type_name(type_name).to_string());
        let mut substitutions = HashMap::new();
        let mut visited = HashSet::new();
        while let Some(type_name) = current.as_deref() {
            if !visited.insert(type_name.to_string()) {
                break;
            }
            if let Some(fields) = self.fields_by_type.get(type_name) {
                for field in fields {
                    if seen_fields.insert(field.java_name.clone()) {
                        out.push(substitute_field_symbol(field, &substitutions));
                    }
                }
            }
            current = self
                .extends_type_by_type
                .get(type_name)
                .and_then(|super_type| {
                    extend_supertype_substitutions(self, type_name, super_type, &mut substitutions)
                });
        }
        out
    }

    fn fields_for_enclosing_types(&self, type_name: &str) -> Vec<FieldSymbol> {
        let mut out = Vec::new();
        let mut seen_fields = HashSet::new();
        let mut current = self
            .outer_by_type
            .get(self.source_type_name(type_name))
            .map(String::as_str);
        let mut visited = HashSet::new();
        while let Some(type_name) = current {
            let type_name = self.source_type_name(type_name);
            if !visited.insert(type_name.to_string()) {
                break;
            }
            for field in self.fields_for_hierarchy(type_name) {
                if seen_fields.insert(field.java_name.clone()) {
                    out.push(field);
                }
            }
            current = self.outer_by_type.get(type_name).map(String::as_str);
        }
        out
    }

    fn source_type_name<'a>(&'a self, type_name: &'a str) -> &'a str {
        self.canonical_type_names
            .get(type_name)
            .map(String::as_str)
            .unwrap_or(type_name)
    }

    fn is_functional_interface_type(&self, type_name: &str) -> bool {
        let source_name = self.source_type_name(type_name);
        self.functional_interfaces.contains(source_name)
            || crate::stdlib::is_known_functional_interface(source_name)
    }

    fn source_functional_interface_method(&self, ty: &JavaType) -> Option<&MethodSymbol> {
        let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = ty else {
            return None;
        };
        let source_name = self.source_type_name(name);
        self.functional_interface_methods.get(source_name)
    }

    fn source_subtype_distance(
        &self,
        actual: &str,
        target: &str,
        external_facts: &FactSet,
    ) -> Option<u32> {
        let target = self.source_type_name(target);
        for (type_name, distance) in self.source_hierarchy_walk(actual) {
            let type_name = self.source_type_name(&type_name);
            if type_name == target {
                return Some(distance);
            }
            if self.fact_subtype_reaches(type_name, target, external_facts) {
                return Some(distance + 2);
            }
            if crate::stdlib::is_known_java_subtype(type_name, target) {
                return Some(distance + 5);
            }
        }

        None
    }

    fn source_hierarchy_walk(&self, actual: &str) -> Vec<(String, u32)> {
        let actual = self.source_type_name(actual).to_string();
        if let Some(walk) = self
            .source_hierarchy_walk_by_type
            .lock()
            .expect("source hierarchy walk cache mutex poisoned")
            .get(&actual)
            .cloned()
        {
            return walk;
        }

        let mut walk = Vec::new();
        let mut pending = vec![(actual.clone(), 0_u32)];
        let mut visited = HashSet::new();
        while let Some((type_name, distance)) = pending.pop() {
            let type_name = self.source_type_name(&type_name).to_string();
            if !visited.insert(type_name.clone()) {
                continue;
            }
            walk.push((type_name.clone(), distance));
            if let Some(super_type) = self.extends_by_type.get(&type_name) {
                pending.push((self.source_type_name(super_type).to_string(), distance + 1));
            }
            if let Some(interfaces) = self.implements_by_type.get(&type_name) {
                for interface in interfaces {
                    pending.push((self.source_type_name(interface).to_string(), distance + 1));
                }
            }
        }

        self.source_hierarchy_walk_by_type
            .lock()
            .expect("source hierarchy walk cache mutex poisoned")
            .insert(actual, walk.clone());
        walk
    }

    fn fact_subtype_reaches(&self, actual: &str, target: &str, external_facts: &FactSet) -> bool {
        let actual_ty = JavaType::Named {
            name: self.source_type_name(actual).to_string(),
            args: Vec::new(),
        };
        let target_ty = JavaType::Named {
            name: self.source_type_name(target).to_string(),
            args: Vec::new(),
        };
        external_facts
            .hierarchy_subtype_policy(&actual_ty, &target_ty)
            .reaches
    }

    fn is_source_enum_type(&self, type_name: &str) -> bool {
        let source_name = self.source_type_name(type_name);
        self.kind_by_type
            .get(source_name)
            .is_some_and(|kind| *kind == TypeKind::Enum)
    }

    fn source_type_kind(&self, type_name: &str) -> Option<TypeKind> {
        let source_name = self.source_type_name(type_name);
        self.kind_by_type.get(source_name).copied()
    }

    fn type_params_for(&self, type_name: &str) -> Option<&[String]> {
        self.type_params_by_type
            .get(self.source_type_name(type_name))
            .map(Vec::as_slice)
    }

    fn type_param_bounds_for(&self, type_name: &str) -> Option<&[Option<JavaType>]> {
        self.type_param_bounds_by_type
            .get(self.source_type_name(type_name))
            .map(Vec::as_slice)
    }

    fn type_param_bound_for(&self, type_name: &str, param_name: &str) -> Option<JavaType> {
        let source_name = self.source_type_name(type_name);
        let params = self.type_params_by_type.get(source_name)?;
        let bounds = self.type_param_bounds_by_type.get(source_name)?;
        let index = params.iter().position(|param| param == param_name)?;
        bounds.get(index).and_then(Clone::clone)
    }

    fn source_type_accepts_actual_type(&self, actual_ty: &JavaType) -> bool {
        let JavaType::Named { name, args } = actual_ty else {
            return false;
        };
        let source_name = self.source_type_name(name);
        if !self.methods_by_type.contains_key(source_name) {
            return false;
        }
        let Some(params) = self.type_params_by_type.get(source_name) else {
            return args.is_empty();
        };
        args.is_empty() || params.len() == args.len()
    }

    fn source_single_arg_supertype_arg(
        &self,
        actual_ty: &JavaType,
        target_type: &str,
    ) -> Option<JavaType> {
        let JavaType::Named { name, args } = actual_ty else {
            return None;
        };
        let mut current = self.source_type_name(name).to_string();
        let mut substitutions = HashMap::new();
        if let Some(params) = self.type_params_for(&current) {
            for (param, arg) in params.iter().zip(args) {
                substitutions.insert(param.clone(), arg.clone());
            }
        }
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current.clone()) {
                return None;
            }
            if current == target_type {
                let args = self
                    .type_params_for(&current)
                    .and_then(|params| params.first())
                    .and_then(|param| substitutions.get(param))
                    .cloned();
                if args.is_some() {
                    return args;
                }
            }
            if let Some(interfaces) = self.implements_type_by_type.get(&current) {
                for interface_ty in interfaces {
                    let substituted =
                        substitute_method_type_variables(interface_ty, &substitutions);
                    if let JavaType::Named {
                        name: interface_name,
                        args,
                    } = &substituted
                        && self.source_type_name(interface_name) == target_type
                        && let Some(arg) = args.first()
                    {
                        return Some(arg.clone());
                    }
                }
            }
            let Some(super_type) = self.extends_type_by_type.get(&current) else {
                return None;
            };
            let Some(super_name) =
                extend_supertype_substitutions(self, &current, super_type, &mut substitutions)
            else {
                return None;
            };
            current = super_name;
        }
    }
}

fn field_name_matches(field: &FieldSymbol, field_name: &str, canonical_field_name: &str) -> bool {
    field.java_name == field_name
        || crate::parser::to_snake_case(&field.java_name) == canonical_field_name
}

fn is_source_functional_interface(ty: &TypeDecl) -> bool {
    if ty.kind != TypeKind::Interface {
        return false;
    }
    let abstract_method_count = ty
        .members
        .iter()
        .filter(|member| match member {
            Member::Method(method) => !method.modifiers.contains(&Modifier::Static),
            _ => false,
        })
        .count();
    abstract_method_count == 1
}

fn extend_supertype_substitutions(
    symbols: &ProgramSymbols,
    _current_type: &str,
    super_type: &JavaType,
    substitutions: &mut HashMap<String, JavaType>,
) -> Option<String> {
    let JavaType::Named { name, args } = super_type else {
        return None;
    };
    let super_name = symbols.source_type_name(name).to_string();
    if let Some(super_params) = symbols
        .methods_by_type
        .get(&super_name)
        .and_then(|_| symbols.type_params_for(&super_name))
    {
        let super_bounds = symbols.type_param_bounds_for(&super_name).unwrap_or(&[]);
        for (index, (param, arg)) in super_params.iter().zip(args).enumerate() {
            let substituted_arg = substitute_method_type_variables(arg, substitutions);
            substitutions.insert(
                param.clone(),
                wildcard_actual_or_declared_bound(&substituted_arg, super_bounds.get(index)),
            );
        }
    }
    Some(super_name)
}

fn substitute_method_symbol(
    method: &MethodSymbol,
    substitutions: &HashMap<String, JavaType>,
) -> MethodSymbol {
    MethodSymbol {
        declaring_type: method.declaring_type.clone(),
        owner_type_params: method.owner_type_params.clone(),
        owner_type_param_bounds: method.owner_type_param_bounds.clone(),
        method_type_params: method.method_type_params.clone(),
        method_type_param_bounds: method.method_type_param_bounds.clone(),
        java_name: method.java_name.clone(),
        rust_name: method.rust_name.clone(),
        params: method
            .params
            .iter()
            .map(|param| Param {
                name: param.name.clone(),
                source_name: param.source_name.clone(),
                ty: substitute_method_type_variables(&param.ty, substitutions),
                is_varargs: param.is_varargs,
                binding: param.binding.clone(),
            })
            .collect(),
        return_type: substitute_method_type_variables(&method.return_type, substitutions),
        is_static: method.is_static,
    }
}

fn substitute_field_symbol(
    field: &FieldSymbol,
    substitutions: &HashMap<String, JavaType>,
) -> FieldSymbol {
    FieldSymbol {
        declaring_type: field.declaring_type.clone(),
        outer_type: field.outer_type.clone(),
        java_name: field.java_name.clone(),
        ty: substitute_method_type_variables(&field.ty, substitutions),
        is_static: field.is_static,
    }
}

fn method_signature_key(method: &MethodSymbol) -> String {
    let mut key = method.java_name.clone();
    key.push('(');
    for param in &method.params {
        key.push_str(&java_type_signature_key(&param.ty));
        key.push(';');
    }
    key.push(')');
    key
}

fn java_type_signature_key(java_ty: &JavaType) -> String {
    match java_ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(inner) => format!("{}[]", java_type_signature_key(inner)),
        JavaType::Named { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    args.iter()
                        .map(java_type_signature_key)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        }
        JavaType::Wildcard(bound) => format!("?{bound:?}"),
        JavaType::Unknown(name) => format!("unknown:{name}"),
    }
}

fn collect_field_env(ty: &TypeDecl, symbols: &ProgramSymbols) -> TypeEnv {
    let mut env = TypeEnv::new();
    for field in symbols.fields_for_hierarchy(&ty.name) {
        env.insert_field(field.java_name, field.ty);
    }
    for field in symbols.fields_for_enclosing_types(&ty.name) {
        if env.field_type(&field.java_name).is_none() {
            env.insert_field(field.java_name, field.ty);
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
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
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
            facts,
            external_facts,
            enclosing_member,
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
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ty,
            ..
        } => {
            resolve_expr_calls(
                expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            apply_target_type_to_constructor_expr(expr, ty, symbols, external_facts);
        }
        Stmt::VarDecl { .. } => {}
        Stmt::Assign { target, value } => {
            resolve_expr_calls(
                target,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                value,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Stmt::Expr(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Stmt::Return(Some(expr)) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_block_calls(
                then_block,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
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
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
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
                    facts,
                    external_facts,
                    enclosing_member,
                );
                env.learn_stmt(stmt);
            }
            if let Some(condition) = condition {
                resolve_expr_calls(
                    condition,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            for expr in update {
                resolve_expr_calls(
                    expr,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            resolve_block_calls(
                body,
                current_type,
                symbols,
                methods,
                constructors,
                &mut env.clone(),
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            resolve_expr_calls(
                iterable,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
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
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Stmt::Switch { selector, cases } => {
            resolve_expr_calls(
                selector,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            for case in cases {
                for label in &mut case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        resolve_expr_calls(
                            expr,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
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
                        facts,
                        external_facts,
                        enclosing_member,
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
                    facts,
                    external_facts,
                    enclosing_member,
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
                facts,
                external_facts,
                enclosing_member,
            );
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.source_name.clone(), ty.clone());
                }
                resolve_block_calls(
                    &mut catch.body,
                    current_type,
                    symbols,
                    methods,
                    constructors,
                    &mut catch_env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
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
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
        }
        Stmt::Throw(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            for arg in args.iter_mut() {
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            if matches!(kind, ConstructorInvocationKind::This) {
                let arg_types = args
                    .iter()
                    .map(|arg| {
                        infer_resolver_expr_type_with_facts(
                            arg,
                            current_type,
                            symbols,
                            env,
                            external_facts,
                        )
                    })
                    .collect::<Vec<_>>();
                match resolve_same_class_constructor(
                    &arg_types,
                    constructors,
                    symbols,
                    external_facts,
                ) {
                    Ok(Some(constructor)) => *resolved = Some(constructor),
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::ConstructorDelegation,
                        target_name: "this".to_string(),
                        receiver_type: Some(JavaType::Named {
                            name: current_type.to_string(),
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error(
                        "resolve_constructor_ambiguous",
                        format!("constructor this: {message}"),
                    ),
                }
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            constructors,
            &mut env.clone(),
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        _ => {}
    }
}

fn apply_target_type_to_constructor_expr(
    expr: &mut Expr,
    target_ty: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    let Expr::New {
        ty,
        args,
        resolved: Some(resolved),
        ..
    } = expr
    else {
        return;
    };
    resolved.constructed_type = inferred_constructed_type(
        ty,
        Some(target_ty),
        &resolved.declaring_type,
        symbols,
        external_facts,
    );
    refresh_constructor_derived_fields(resolved, None, symbols, external_facts);
    apply_constructor_param_targets(args, resolved, symbols, external_facts);
}

fn apply_constructor_param_targets(
    args: &mut [Expr],
    constructor: &ResolvedConstructor,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    for (arg, param) in args.iter_mut().zip(&constructor.params) {
        apply_target_type_to_constructor_expr(arg, &param.ty, symbols, external_facts);
    }
}

fn constructor_params_for_constructed_type(
    constructor: &ResolvedConstructor,
    symbols: &ProgramSymbols,
) -> Vec<Param> {
    let JavaType::Named { name, args } = &constructor.constructed_type else {
        return constructor.params.clone();
    };
    let Some(type_params) = symbols.type_params_for(name) else {
        return constructor.params.clone();
    };
    let substitutions = type_params
        .iter()
        .zip(args)
        .map(|(param, arg)| (param.clone(), arg.clone()))
        .collect::<HashMap<_, _>>();
    constructor
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(&param.ty, &substitutions),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect()
}

fn refresh_constructor_derived_fields(
    constructor: &mut ResolvedConstructor,
    arg_types: Option<&[JavaType]>,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    let recovered_arg_types = arg_types
        .is_none()
        .then(|| constructor_arg_actual_types(constructor))
        .flatten();
    let effective_arg_types = arg_types.or(recovered_arg_types.as_deref());

    constructor.params = constructor_params_for_constructed_type(constructor, symbols);

    if let Some(arg_types) = effective_arg_types {
        let param_types = constructor
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect::<Vec<_>>();
        constructor.arg_coercions = java_coercion_evidence_for_params(
            &constructor.params,
            &param_types,
            arg_types,
            symbols,
            external_facts,
        );
    }
}

fn constructor_arg_actual_types(constructor: &ResolvedConstructor) -> Option<Vec<JavaType>> {
    let arg_count = constructor
        .arg_coercions
        .iter()
        .map(|coercion| coercion.arg_index)
        .max()
        .map_or(0, |index| index + 1);
    let mut arg_types = vec![None; arg_count];
    for coercion in &constructor.arg_coercions {
        if let Some(slot) = arg_types.get_mut(coercion.arg_index) {
            *slot = Some(coercion.actual.clone());
        }
    }
    arg_types.into_iter().collect()
}

fn inferred_constructed_type(
    parsed_ty: &JavaType,
    target_ty: Option<&JavaType>,
    declaring_type: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> JavaType {
    let JavaType::Named { name, args } = parsed_ty else {
        return parsed_ty.clone();
    };
    if !args.is_empty() {
        return parsed_ty.clone();
    }
    let Some(JavaType::Named {
        name: target_name,
        args: target_args,
    }) = target_ty
    else {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    };
    if target_args.is_empty() {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    }
    let source_name = symbols.source_type_name(name);
    let source_target = symbols.source_type_name(target_name);
    if source_name != source_target
        && !crate::stdlib::is_known_java_subtype(source_name, source_target)
        && symbols
            .source_subtype_distance(source_name, source_target, external_facts)
            .is_none()
    {
        return JavaType::Named {
            name: declaring_type.to_string(),
            args: Vec::new(),
        };
    }
    JavaType::Named {
        name: declaring_type.to_string(),
        args: target_args.clone(),
    }
}

fn resolve_expr_calls(
    expr: &mut Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            resolve_expr_calls(
                receiver,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            record_unresolved_field_access(
                expr,
                current_type,
                symbols,
                env,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::ArrayAccess(array, index) => {
            resolve_expr_calls(
                array,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                index,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr_calls(
                left,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                right,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Unary { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            resolve_expr_calls(
                condition,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                then_expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
            resolve_expr_calls(
                else_expr,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            if let Some(receiver) = receiver {
                resolve_expr_calls(
                    receiver,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            let receiver_type = receiver
                .as_deref()
                .map(|receiver| infer_resolver_expr_type(receiver, current_type, symbols, env));
            let defer_stream_lambda_arg =
                should_defer_stream_lambda_arg_resolution(receiver_type.as_ref(), name, args);
            for (arg_index, arg) in args.iter_mut().enumerate() {
                if defer_stream_lambda_arg && arg_index == 0 {
                    continue;
                }
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }

            resolve_stream_map_lambda_arg(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                methods,
                env,
                diagnostics,
                facts,
                external_facts,
                enclosing_member,
            );

            let arg_types = args
                .iter()
                .map(|arg| {
                    infer_resolver_expr_type_with_facts(
                        arg,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    )
                })
                .collect::<Vec<_>>();
            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                if receiver.is_none()
                    && let Some(enum_call) =
                        source_enum_synthetic_call(current_type, name, arg_types.len(), symbols)
                {
                    *resolved = Some(enum_call);
                    return;
                }
                let unqualified_methods = if receiver.is_none() {
                    symbols.methods_for_unqualified_call(current_type)
                } else {
                    methods.to_vec()
                };
                match resolve_same_class_call(
                    name,
                    &arg_types,
                    &unqualified_methods,
                    symbols,
                    external_facts,
                    None,
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: if receiver.is_none()
                            && crate::stdlib::is_known_receiverless_static_utility_method(
                                name,
                                arg_types.len(),
                            ) {
                            None
                        } else {
                            Some(JavaType::Named {
                                name: current_type.to_string(),
                                args: Vec::new(),
                            })
                        },
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some(receiver_ty) = receiver
                .as_deref()
                .and_then(|receiver| source_static_type_receiver(receiver, symbols))
            {
                let receiver_methods = symbols
                    .methods_for_hierarchy(&receiver_ty)
                    .into_iter()
                    .filter(|method| method.is_static)
                    .collect::<Vec<_>>();
                match resolve_same_class_call(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    None,
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: Some(JavaType::Named {
                            name: receiver_ty,
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else if let Some((receiver_ty, receiver_actual_ty)) = receiver
                .as_deref()
                .and_then(|receiver| source_receiver_type(receiver, current_type, symbols, env))
            {
                let receiver_methods = symbols.methods_for_hierarchy(&receiver_ty);
                match resolve_same_class_call(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    Some(&receiver_actual_ty),
                ) {
                    Ok(Some(selection)) => {
                        resolve_target_typed_lambda_args(
                            args,
                            &selection.params,
                            current_type,
                            symbols,
                            methods,
                            env,
                            diagnostics,
                            facts,
                            external_facts,
                            enclosing_member,
                        );
                        *resolved = Some(selection.call);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::MethodCall,
                        target_name: name.clone(),
                        receiver_type: Some(JavaType::Named {
                            name: receiver_ty,
                            args: Vec::new(),
                        }),
                        arg_types,
                        is_anonymous_class: false,
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            } else {
                let receiver_type = receiver
                    .as_deref()
                    .map(|receiver| unresolved_receiver_type(receiver, current_type, symbols, env));
                facts.unresolved_target_facts.push(UnresolvedTargetFact {
                    enclosing_type: current_type.to_string(),
                    enclosing_member: enclosing_member.map(str::to_string),
                    kind: UnresolvedTargetKind::MethodCall,
                    target_name: name.clone(),
                    receiver_type,
                    arg_types,
                    is_anonymous_class: false,
                    provenance: FactProvenance::ParsedSource,
                });
            }
        }
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => {
            for arg in args.iter_mut() {
                resolve_expr_calls(
                    arg,
                    current_type,
                    symbols,
                    methods,
                    env,
                    diagnostics,
                    facts,
                    external_facts,
                    enclosing_member,
                );
            }
            if let JavaType::Named { name, .. } | JavaType::Unknown(name) = ty {
                let arg_types = args
                    .iter()
                    .map(|arg| {
                        infer_resolver_expr_type_with_facts(
                            arg,
                            current_type,
                            symbols,
                            env,
                            external_facts,
                        )
                    })
                    .collect::<Vec<_>>();
                let constructor_candidates = symbols.constructors_for_new(name, current_type);
                let selection = if body.is_some() {
                    resolve_anonymous_class_constructor(
                        name,
                        &arg_types,
                        &constructor_candidates,
                        symbols,
                        external_facts,
                    )
                } else {
                    resolve_same_class_constructor(
                        &arg_types,
                        &constructor_candidates,
                        symbols,
                        external_facts,
                    )
                };
                match selection {
                    Ok(Some(mut constructor)) => {
                        constructor.constructed_type = inferred_constructed_type(
                            ty,
                            None,
                            &constructor.declaring_type,
                            symbols,
                            external_facts,
                        );
                        refresh_constructor_derived_fields(
                            &mut constructor,
                            Some(&arg_types),
                            symbols,
                            external_facts,
                        );
                        apply_constructor_param_targets(
                            args,
                            &constructor,
                            symbols,
                            external_facts,
                        );
                        *resolved = Some(constructor);
                    }
                    Ok(None) => facts.unresolved_target_facts.push(UnresolvedTargetFact {
                        enclosing_type: current_type.to_string(),
                        enclosing_member: enclosing_member.map(str::to_string),
                        kind: UnresolvedTargetKind::ConstructorCall,
                        target_name: name.clone(),
                        receiver_type: Some(ty.clone()),
                        arg_types,
                        is_anonymous_class: body.is_some(),
                        provenance: FactProvenance::ParsedSource,
                    }),
                    Err(message) => diagnostics.error(
                        "resolve_constructor_ambiguous",
                        format!("constructor {name}: {message}"),
                    ),
                }
            }
            if let Some(members) = body {
                for member in members {
                    match member {
                        Member::Field(field) => {
                            if let Some(initializer) = &mut field.initializer {
                                resolve_expr_calls(
                                    initializer,
                                    current_type,
                                    symbols,
                                    methods,
                                    env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&field.name),
                                );
                            }
                        }
                        Member::Method(method) => {
                            if let Some(body) = &mut method.body {
                                let mut body_env = env.clone();
                                for param in &method.params {
                                    body_env.insert_local(param.name.clone(), param.ty.clone());
                                }
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut body_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&method.name),
                                );
                            }
                        }
                        Member::Constructor(ctor) => {
                            if let Some(body) = &mut ctor.body {
                                let mut body_env = env.clone();
                                for param in &ctor.params {
                                    body_env.insert_local(param.name.clone(), param.ty.clone());
                                }
                                resolve_block_calls(
                                    body,
                                    current_type,
                                    symbols,
                                    methods,
                                    &[],
                                    &mut body_env,
                                    diagnostics,
                                    facts,
                                    external_facts,
                                    Some(&ctor.name),
                                );
                            }
                        }
                        Member::Type(ty) => {
                            // A local type declared inside a body: only its own
                            // members are in play, so its counts stand alone.
                            let mut local_counts = ProgramMethodCounts::new();
                            collect_program_method_counts(ty, &mut local_counts);
                            assign_type_rust_names(ty, diagnostics, &local_counts);
                        }
                        _ => {}
                    }
                }
            }
        }
        Expr::Cast { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::InstanceOf { expr, .. } => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        Expr::Lambda { .. } => {}
        _ => {}
    }
}

fn should_defer_stream_lambda_arg_resolution(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> bool {
    if !is_stream_lambda_arg_name(name) || args.len() != 1 {
        return false;
    }
    matches!(
        receiver_type,
        Some(JavaType::Named {
            name: receiver_name,
            ..
        }) if receiver_name == "Stream"
    ) && matches!(args.first(), Some(Expr::Lambda { .. }))
}

#[allow(clippy::too_many_arguments)]
fn resolve_stream_map_lambda_arg(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &mut [Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    if !is_stream_lambda_arg_name(name) || args.len() != 1 {
        return;
    }
    let Some(JavaType::Named {
        name: receiver_name,
        args: receiver_args,
    }) = receiver_type
    else {
        return;
    };
    if receiver_name != "Stream" {
        return;
    }
    let Some(stream_element) = receiver_args.first() else {
        return;
    };
    resolve_lambda_with_single_param_type(
        &mut args[0],
        stream_element,
        current_type,
        symbols,
        methods,
        env,
        diagnostics,
        facts,
        external_facts,
        enclosing_member,
    );
}

fn is_stream_lambda_mapper_name(name: &str) -> bool {
    matches!(name, "map" | "mapToInt" | "mapToLong" | "mapToDouble")
}

fn is_stream_lambda_arg_name(name: &str) -> bool {
    is_stream_lambda_mapper_name(name) || name == "flatMap"
}

#[allow(clippy::too_many_arguments)]
fn resolve_target_typed_lambda_args(
    args: &mut [Expr],
    params: &[Param],
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    for (arg, param) in args.iter_mut().zip(params) {
        resolve_lambda_with_expected_type(
            arg,
            &param.ty,
            current_type,
            symbols,
            methods,
            env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_lambda_with_single_param_type(
    expr: &mut Expr,
    expected_param_ty: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    let Expr::Lambda { params, body } = expr else {
        return;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| expected_param_ty.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    match body {
        LambdaBody::Expr(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            &lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        LambdaBody::Block(block) => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            &[],
            &mut lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_lambda_with_expected_type(
    expr: &mut Expr,
    expected_ty: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    let Expr::Lambda { params, body } = expr else {
        return;
    };
    let Some(functional_method) = symbols.source_functional_interface_method(expected_ty) else {
        return;
    };
    let substitutions = functional_interface_type_substitutions(expected_ty, symbols);

    let mut lambda_env = env.clone();
    for (lambda_param, method_param) in params.iter().zip(&functional_method.params) {
        let method_param_ty = substitute_method_type_variables(&method_param.ty, &substitutions);
        let param_ty = lambda_param.ty.clone().unwrap_or(method_param_ty);
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    match body {
        LambdaBody::Expr(expr) => resolve_expr_calls(
            expr,
            current_type,
            symbols,
            methods,
            &lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
        LambdaBody::Block(block) => resolve_block_calls(
            block,
            current_type,
            symbols,
            methods,
            &[],
            &mut lambda_env,
            diagnostics,
            facts,
            external_facts,
            enclosing_member,
        ),
    }
}

fn record_unresolved_field_access(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    facts: &mut FactSet,
    external_facts: &FactSet,
    enclosing_member: Option<&str>,
) {
    let Expr::FieldAccess(receiver, field) = expr else {
        return;
    };
    if infer_resolver_member_access_type_with_facts(
        receiver,
        field,
        current_type,
        symbols,
        env,
        external_facts,
    )
    .is_some()
    {
        return;
    }
    if matches!(receiver.as_ref(), Expr::This) && env.field_type(field).is_some() {
        return;
    }
    if qualified_this_type_name(receiver, field, symbols).is_some() {
        return;
    }
    if let Some(receiver_type_name) = field_receiver_type_name(receiver, current_type, symbols, env)
        && symbols
            .field_for_from(&receiver_type_name, field, current_type)
            .is_some_and(|symbol| field_access_is_allowed(receiver, symbol, symbols, env))
    {
        return;
    }
    if let Some(receiver_type_name) = field_receiver_type_name(receiver, current_type, symbols, env)
        && symbols
            .external_super_field_type_for(&receiver_type_name, field)
            .is_some()
    {
        return;
    }
    if type_qualified_static_field(receiver, field, current_type, symbols).is_some() {
        return;
    }
    if is_package_qualified_type_fragment(expr, env) {
        return;
    }
    facts.unresolved_target_facts.push(UnresolvedTargetFact {
        enclosing_type: current_type.to_string(),
        enclosing_member: enclosing_member.map(str::to_string),
        kind: UnresolvedTargetKind::FieldAccess,
        target_name: field.clone(),
        receiver_type: Some(unresolved_receiver_type(
            receiver,
            current_type,
            symbols,
            env,
        )),
        arg_types: Vec::new(),
        is_anonymous_class: false,
        provenance: FactProvenance::ParsedSource,
    });
}

fn is_package_qualified_type_fragment(expr: &Expr, env: &TypeEnv) -> bool {
    let Some(segments) = field_access_segments(expr) else {
        return false;
    };
    let [root, rest @ ..] = segments.as_slice() else {
        return false;
    };
    if rest.is_empty() || env.local_type(root).is_some() || env.field_type(root).is_some() {
        return false;
    }
    let Some(last) = segments.last() else {
        return false;
    };
    let prefix = &segments[..segments.len() - 1];
    if segments.iter().all(|segment| is_package_segment(segment)) {
        return true;
    }
    segments.len() >= 3
        && prefix.iter().all(|segment| is_package_segment(segment))
        && starts_like_type_name(last)
}

fn field_access_segments(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Var(name) => Some(vec![name.clone()]),
        Expr::FieldAccess(receiver, field) => {
            let mut segments = field_access_segments(receiver)?;
            segments.push(field.clone());
            Some(segments)
        }
        _ => None,
    }
}

fn is_package_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn starts_like_type_name(segment: &str) -> bool {
    segment
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn field_receiver_type_name(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<String> {
    match receiver {
        Expr::This | Expr::Super => Some(current_type.to_string()),
        Expr::FieldAccess(inner_receiver, field_name)
            if qualified_this_type_name(inner_receiver, field_name, symbols).is_some() =>
        {
            qualified_this_type_name(inner_receiver, field_name, symbols)
        }
        Expr::Var(name) => env.local_type(name).map(java_type_simple_name).or_else(|| {
            env.field_type(name)
                .map(java_type_simple_name)
                .or_else(|| Some(name.clone()))
        }),
        Expr::FieldAccess(inner_receiver, field_name) => {
            if let Some(field) =
                type_qualified_static_field(inner_receiver, field_name, current_type, symbols)
            {
                return Some(java_type_simple_name(&field.ty));
            }
            let owner_type = field_receiver_type_name(inner_receiver, current_type, symbols, env)?;
            symbols
                .field_for_from(&owner_type, field_name, current_type)
                .map(|field| java_type_simple_name(&field.ty))
                .or_else(|| {
                    Some(java_type_simple_name(&infer_resolver_expr_type(
                        receiver,
                        current_type,
                        symbols,
                        env,
                    )))
                })
        }
        other => Some(java_type_simple_name(&infer_resolver_expr_type(
            other,
            current_type,
            symbols,
            env,
        ))),
    }
}

fn qualified_this_type_name(
    receiver: &Expr,
    field_name: &str,
    symbols: &ProgramSymbols,
) -> Option<String> {
    if field_name != "this" {
        return None;
    }
    let Expr::Var(name) = receiver else {
        return None;
    };
    let source_name = symbols.source_type_name(name);
    if source_name == name && !symbols.fields_by_type.contains_key(source_name) {
        None
    } else {
        Some(source_name.to_string())
    }
}

fn field_access_is_allowed(
    receiver: &Expr,
    symbol: &FieldSymbol,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> bool {
    match receiver {
        Expr::Var(name)
            if env.local_type(name).is_none()
                && env.field_type(name).is_none()
                && symbols.source_type_name(name) == symbol.declaring_type =>
        {
            symbol.is_static
        }
        _ => true,
    }
}

fn type_qualified_static_field<'a>(
    receiver: &Expr,
    field_name: &str,
    current_type: &str,
    symbols: &'a ProgramSymbols,
) -> Option<&'a FieldSymbol> {
    let Expr::Var(name) = receiver else {
        return None;
    };
    let source_name = symbols.source_type_name(name);
    symbols.static_field_for_from(source_name, field_name, current_type)
}

fn java_type_simple_name(java_ty: &JavaType) -> String {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::String => "String".to_string(),
        JavaType::Array(_) => "Array".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}"),
        JavaType::Void => "void".to_string(),
        JavaType::Wildcard(_) => "wildcard".to_string(),
    }
}

fn nested_simple_type_name(type_name: &str) -> &str {
    type_name.rsplit('.').next().unwrap_or(type_name)
}

fn unresolved_receiver_type(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> JavaType {
    match receiver {
        Expr::This | Expr::Super => JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        },
        Expr::FieldAccess(inner_receiver, field_name)
            if qualified_this_type_name(inner_receiver, field_name, symbols).is_some() =>
        {
            JavaType::Named {
                name: qualified_this_type_name(inner_receiver, field_name, symbols).unwrap(),
                args: Vec::new(),
            }
        }
        other => infer_resolver_expr_type(other, current_type, symbols, env),
    }
}

fn functional_interface_type_substitutions(
    expected_ty: &JavaType,
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let JavaType::Named { name, args } = expected_ty else {
        return HashMap::new();
    };
    symbols
        .type_params_for(name)
        .unwrap_or(&[])
        .iter()
        .zip(args)
        .map(|(param, arg)| (param.clone(), arg.clone()))
        .collect()
}

fn infer_resolver_expr_type(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> JavaType {
    infer_resolver_expr_type_with_facts(expr, current_type, symbols, env, &FactSet::default())
}

fn infer_resolver_expr_type_with_facts(
    expr: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> JavaType {
    match expr {
        Expr::Call {
            resolved: Some(call),
            ..
        } => call.return_type.clone(),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: None,
        } => {
            if name == "nonNull" && !args.is_empty() {
                return infer_resolver_expr_type_with_facts(
                    &args[0],
                    current_type,
                    symbols,
                    env,
                    external_facts,
                );
            }
            let receiver_type = receiver.as_deref().map(|receiver| {
                infer_resolver_expr_type_with_facts(
                    receiver,
                    current_type,
                    symbols,
                    env,
                    external_facts,
                )
            });
            if let Some(java_ty) = infer_stream_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_method_reference_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_lambda_flat_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = infer_stream_method_reference_flat_map_return_type(
                receiver_type.as_ref(),
                name,
                args,
            ) {
                return java_ty;
            }
            if let Some(java_ty) =
                infer_stream_to_array_generator_return_type(receiver_type.as_ref(), name, args)
            {
                return java_ty;
            }
            let arg_types = args
                .iter()
                .map(|arg| {
                    infer_resolver_expr_type_with_facts(
                        arg,
                        current_type,
                        symbols,
                        env,
                        external_facts,
                    )
                })
                .collect::<Vec<_>>();
            if receiver.is_none()
                && let Some(enum_call) =
                    source_enum_synthetic_call(current_type, name, arg_types.len(), symbols)
            {
                return enum_call.return_type;
            }
            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                let methods = if receiver.is_none() {
                    symbols.methods_for_unqualified_call(current_type)
                } else {
                    symbols.methods_for_hierarchy(current_type)
                };
                if let Ok(Some(selection)) = resolve_same_class_call(
                    name,
                    &arg_types,
                    &methods,
                    symbols,
                    external_facts,
                    None,
                ) {
                    return selection.call.return_type;
                }
            }
            if let Some((receiver_ty, receiver_actual_ty)) = receiver
                .as_deref()
                .and_then(|receiver| source_receiver_type(receiver, current_type, symbols, env))
            {
                let receiver_methods = symbols.methods_for_hierarchy(&receiver_ty);
                if let Ok(Some(selection)) = resolve_same_class_call(
                    name,
                    &arg_types,
                    &receiver_methods,
                    symbols,
                    external_facts,
                    Some(&receiver_actual_ty),
                ) {
                    return selection.call.return_type;
                }
            }
            if let Some(java_ty) = known_resolver_external_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
                symbols,
            ) {
                return java_ty;
            }
            if let Some(java_ty) = external_fact_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
                external_facts,
            ) {
                return java_ty;
            }
            crate::stdlib::known_external_method_return_type_for_call(
                receiver_type.as_ref(),
                name,
                &arg_types,
            )
            .unwrap_or_else(|| JavaType::Unknown(name.clone()))
        }
        Expr::This | Expr::Super => JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        },
        Expr::FieldAccess(receiver, field) => {
            if let Some(java_ty) = infer_resolver_member_access_type_with_facts(
                receiver,
                field,
                current_type,
                symbols,
                env,
                external_facts,
            ) {
                return java_ty;
            }
            if let Some(type_name) = qualified_this_type_name(receiver, field, symbols) {
                return JavaType::Named {
                    name: type_name,
                    args: Vec::new(),
                };
            }
            if let Some(symbol) =
                type_qualified_static_field(receiver, field, current_type, symbols)
            {
                return symbol.ty.clone();
            }
            if let Some(receiver_type_name) =
                field_receiver_type_name(receiver, current_type, symbols, env)
                && let Some(symbol) =
                    symbols.field_for_from(&receiver_type_name, field, current_type)
                && field_access_is_allowed(receiver, symbol, symbols, env)
            {
                return symbol.ty.clone();
            }
            if let Some(receiver_type_name) =
                field_receiver_type_name(receiver, current_type, symbols, env)
                && let Some(java_ty) =
                    symbols.external_super_field_type_for(&receiver_type_name, field)
            {
                return java_ty;
            }
            infer_expr_type(expr, env)
        }
        Expr::ArrayAccess(array, _) => crate::types::sequence_element_type(
            &infer_resolver_expr_type_with_facts(array, current_type, symbols, env, external_facts),
        )
        .unwrap_or_else(|| infer_expr_type(expr, env)),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => resolver_common_type(
            &infer_resolver_expr_type_with_facts(
                then_expr,
                current_type,
                symbols,
                env,
                external_facts,
            ),
            &infer_resolver_expr_type_with_facts(
                else_expr,
                current_type,
                symbols,
                env,
                external_facts,
            ),
            symbols,
            external_facts,
        ),
        Expr::New {
            resolved: Some(resolved),
            ..
        } => resolved.constructed_type.clone(),
        Expr::New { ty, .. } | Expr::Cast { ty, .. } => ty.clone(),
        Expr::ClassLiteral(ty) => crate::types::class_literal_type(ty),
        other => infer_expr_type(other, env),
    }
}

fn source_enum_synthetic_call(
    current_type: &str,
    name: &str,
    arg_count: usize,
    symbols: &ProgramSymbols,
) -> Option<ResolvedCall> {
    if !symbols.is_source_enum_type(current_type) {
        return None;
    }
    let return_type = match (name, arg_count) {
        ("values", 0) => JavaType::Array(Box::new(JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        })),
        ("valueOf", 1) => JavaType::Named {
            name: current_type.to_string(),
            args: Vec::new(),
        },
        _ => return None,
    };
    Some(ResolvedCall {
        declaring_type: current_type.to_string(),
        rust_name: crate::parser::to_snake_case(name),
        kind: CallKind::Static,
        return_type,
        arg_target_types: if name == "valueOf" && arg_count == 1 {
            vec![JavaType::String]
        } else {
            Vec::new()
        },
        arg_coercions: Vec::new(),
    })
}

fn source_receiver_type(
    receiver: &Expr,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
) -> Option<(String, JavaType)> {
    let receiver_actual_ty = infer_resolver_expr_type(receiver, current_type, symbols, env);
    if let JavaType::Named { name, args } = &receiver_actual_ty
        && args.is_empty()
        && let Some(bound) = env
            .type_param_bound(name)
            .cloned()
            .or_else(|| symbols.type_param_bound_for(current_type, name))
        && let JavaType::Named {
            name: bound_name, ..
        } = &bound
    {
        return Some((symbols.source_type_name(bound_name).to_string(), bound));
    }
    source_receiver_type_from_actual(receiver_actual_ty, symbols)
}

fn source_static_type_receiver(receiver: &Expr, symbols: &ProgramSymbols) -> Option<String> {
    let Expr::Var(name) = receiver else {
        return None;
    };
    let source_name = symbols.source_type_name(name);
    symbols
        .methods_by_type
        .contains_key(source_name)
        .then(|| source_name.to_string())
}

fn source_receiver_type_from_actual(
    receiver_actual_ty: JavaType,
    symbols: &ProgramSymbols,
) -> Option<(String, JavaType)> {
    match &receiver_actual_ty {
        JavaType::Named { name, args } => {
            let source_name = symbols.source_type_name(name).to_string();
            if !args.is_empty() && !symbols.source_type_accepts_actual_type(&receiver_actual_ty) {
                return None;
            }
            symbols
                .methods_by_type
                .contains_key(&source_name)
                .then_some((source_name, receiver_actual_ty))
        }
        JavaType::Unknown(name) => {
            let source_name = symbols.source_type_name(name).to_string();
            symbols
                .methods_by_type
                .contains_key(&source_name)
                .then_some((source_name, receiver_actual_ty))
        }
        _ => None,
    }
}

fn infer_stream_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if !is_stream_lambda_mapper_name(name) || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| stream_element.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    let mapped = match body {
        LambdaBody::Expr(expr) => infer_resolver_expr_type_with_facts(
            expr,
            current_type,
            symbols,
            &lambda_env,
            external_facts,
        ),
        LambdaBody::Block(_) => JavaType::Unknown("lambda".to_string()),
    };

    Some(match name {
        "mapToInt" => JavaType::Named {
            name: "IntStream".to_string(),
            args: Vec::new(),
        },
        "mapToLong" => JavaType::Named {
            name: "LongStream".to_string(),
            args: Vec::new(),
        },
        "mapToDouble" => JavaType::Named {
            name: "DoubleStream".to_string(),
            args: Vec::new(),
        },
        _ => JavaType::Named {
            name: "Stream".to_string(),
            args: vec![mapped],
        },
    })
}

fn known_resolver_external_method_return_type_for_call(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
) -> Option<JavaType> {
    if !matches!(
        receiver_type,
        Some(JavaType::Named { name, .. } | JavaType::Unknown(name))
            if crate::javalibs::is_broad_utils_type_name(name)
    ) || name != "stream"
        || arg_types.len() != 1
    {
        return None;
    }
    let element = symbols.source_single_arg_supertype_arg(&arg_types[0], "Iterable")?;
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    })
}

fn infer_stream_method_reference_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if name != "map" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };

    let mapped = infer_method_reference_return_type(
        target,
        method,
        stream_element,
        current_type,
        symbols,
        env,
        external_facts,
    )?;
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![mapped],
    })
}

fn infer_stream_lambda_flat_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if name != "flatMap" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };

    let mut lambda_env = env.clone();
    if let Some(lambda_param) = params.first() {
        let param_ty = lambda_param
            .ty
            .clone()
            .unwrap_or_else(|| stream_element.clone());
        lambda_env.insert_local(lambda_param.name.clone(), param_ty);
    }

    let LambdaBody::Expr(expr) = body else {
        return None;
    };
    let JavaType::Named {
        name: body_name,
        args: body_args,
    } = infer_resolver_expr_type_with_facts(
        expr,
        current_type,
        symbols,
        &lambda_env,
        external_facts,
    )
    else {
        return None;
    };
    if body_name != "Stream" {
        return None;
    }
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![body_args.first()?.clone()],
    })
}

fn infer_stream_method_reference_flat_map_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> Option<JavaType> {
    if name != "flatMap" || args.len() != 1 {
        return None;
    }
    let stream_element = match receiver_type {
        Some(JavaType::Named { name, args }) if name == "Stream" => args.first()?,
        _ => return None,
    };
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };
    if method != "stream" || !matches!(target.as_str(), "Collection" | "Iterable") {
        return None;
    }
    let element = match stream_element {
        JavaType::Named { args, .. } => args.first()?.clone(),
        _ => return None,
    };
    Some(JavaType::Named {
        name: "Stream".to_string(),
        args: vec![element],
    })
}

fn infer_stream_to_array_generator_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    args: &[Expr],
) -> Option<JavaType> {
    if name != "toArray" || args.len() != 1 {
        return None;
    }
    match receiver_type {
        Some(JavaType::Named { name, .. }) if name == "Stream" => {}
        _ => return None,
    }
    let Expr::MethodReference { target, method } = &args[0] else {
        return None;
    };
    if method != "new" {
        return None;
    }
    let java_ty = crate::parser::parse_type(target);
    matches!(java_ty, JavaType::Array(_)).then_some(java_ty)
}

fn infer_method_reference_return_type(
    target: &str,
    method: &str,
    stream_element: &JavaType,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if target == "String" && method == "valueOf" {
        return Some(JavaType::String);
    }

    if matches!(target, "this" | "super") {
        let methods = symbols.methods_for_hierarchy(current_type);
        let selection = resolve_same_class_call(
            method,
            std::slice::from_ref(stream_element),
            &methods,
            symbols,
            external_facts,
            None,
        )
        .ok()
        .flatten()
        .or_else(|| {
            let snake_case = crate::parser::to_snake_case(method);
            (snake_case != method)
                .then(|| {
                    resolve_same_class_call(
                        &snake_case,
                        std::slice::from_ref(stream_element),
                        &methods,
                        symbols,
                        external_facts,
                        None,
                    )
                    .ok()
                    .flatten()
                })
                .flatten()
        })?;
        return Some(selection.call.return_type);
    }

    if let Some(receiver_type) = method_reference_target_type(target, env) {
        let arg_types = std::slice::from_ref(stream_element);
        if let Some(java_ty) = crate::stdlib::known_external_method_return_type_for_call(
            Some(&receiver_type),
            method,
            arg_types,
        ) {
            return Some(java_ty);
        }

        let receiver_name = java_type_simple_name(&receiver_type);
        let receiver_methods = symbols.methods_for_hierarchy(&receiver_name);
        if let Ok(Some(selection)) = resolve_same_class_call(
            method,
            arg_types,
            &receiver_methods,
            symbols,
            external_facts,
            Some(&receiver_type),
        ) {
            return Some(selection.call.return_type);
        }
    }

    let receiver_type = if matches!(target, "this" | "super") {
        stream_element.clone()
    } else {
        JavaType::Named {
            name: target.to_string(),
            args: Vec::new(),
        }
    };

    if let Some(java_ty) = crate::stdlib::known_external_method_return_type_for_receiver(
        Some(&receiver_type),
        method,
        0,
    ) {
        return Some(java_ty);
    }

    let receiver_name = java_type_simple_name(&receiver_type);
    symbols
        .methods_for_hierarchy(&receiver_name)
        .into_iter()
        .find(|candidate| {
            candidate.java_name == crate::parser::to_snake_case(method)
                && candidate.params.is_empty()
        })
        .map(|candidate| candidate.return_type)
}

fn method_reference_target_type(target: &str, env: &TypeEnv) -> Option<JavaType> {
    env.local_type(target)
        .or_else(|| env.field_type(target))
        .or_else(|| {
            let normalized = crate::parser::to_snake_case(target);
            (normalized != target)
                .then(|| {
                    env.local_type(&normalized)
                        .or_else(|| env.field_type(&normalized))
                })
                .flatten()
        })
        .cloned()
}

fn resolver_common_type(
    left: &JavaType,
    right: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> JavaType {
    if left == right {
        return left.clone();
    }
    if is_null_type(left) && !matches!(right, JavaType::Primitive(_)) {
        return right.clone();
    }
    if is_null_type(right) && !matches!(left, JavaType::Primitive(_)) {
        return left.clone();
    }
    if java_type_assignable(left, right, symbols, external_facts) {
        return right.clone();
    }
    if java_type_assignable(right, left, symbols, external_facts) {
        return left.clone();
    }
    if let Some(common) = known_family_common_type(left, right, symbols, external_facts) {
        return common;
    }
    JavaType::Unknown("common_type".to_string())
}

fn is_null_type(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Unknown(name) if name == "null")
}

fn known_family_common_type(
    left: &JavaType,
    right: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let common_families = ["List", "Set", "Collection", "Iterable", "Map", "Object"];
    common_families
        .iter()
        .find_map(|family| common_family_type(left, right, family, symbols, external_facts))
}

fn common_family_type(
    left: &JavaType,
    right: &JavaType,
    family: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let target = JavaType::Named {
        name: family.to_string(),
        args: common_family_args(left, right, family, symbols, external_facts)?,
    };
    java_type_assignable(left, &target, symbols, external_facts)
        .then_some(())
        .and_then(|_| {
            java_type_assignable(right, &target, symbols, external_facts).then_some(target)
        })
}

fn common_family_args(
    left: &JavaType,
    right: &JavaType,
    family: &str,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<Vec<JavaType>> {
    match family {
        "List" | "Set" | "Collection" | "Iterable" => {
            let left_element = single_generic_arg(left)?;
            let right_element = single_generic_arg(right)?;
            Some(vec![resolver_common_type(
                left_element,
                right_element,
                symbols,
                external_facts,
            )])
        }
        "Map" => {
            let (left_key, left_value) = two_generic_args(left)?;
            let (right_key, right_value) = two_generic_args(right)?;
            Some(vec![
                resolver_common_type(left_key, right_key, symbols, external_facts),
                resolver_common_type(left_value, right_value, symbols, external_facts),
            ])
        }
        "Object" => Some(Vec::new()),
        _ => None,
    }
}

fn single_generic_arg(ty: &JavaType) -> Option<&JavaType> {
    match ty {
        JavaType::Named { args, .. } => args.first(),
        _ => None,
    }
}

fn two_generic_args(ty: &JavaType) -> Option<(&JavaType, &JavaType)> {
    match ty {
        JavaType::Named { args, .. } => match args.as_slice() {
            [left, right] => Some((left, right)),
            _ => None,
        },
        _ => None,
    }
}

fn infer_resolver_member_access_type_with_facts(
    receiver: &Expr,
    member: &str,
    current_type: &str,
    symbols: &ProgramSymbols,
    env: &TypeEnv,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if let Some(java_ty) = crate::types::infer_member_access_type(receiver, member, env) {
        return Some(java_ty);
    }
    let receiver_ty =
        infer_resolver_expr_type_with_facts(receiver, current_type, symbols, env, external_facts);
    if member == "length" && matches!(receiver_ty, JavaType::Array(_) | JavaType::String) {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if member == "length" && matches!(receiver_ty, JavaType::Unknown(_)) {
        return Some(JavaType::Primitive(crate::java_ir::PrimitiveType::Int));
    }
    if let Some((receiver_type_name, _)) =
        source_receiver_type(receiver, current_type, symbols, env)
        && let Some(symbol) = symbols.field_for_from(&receiver_type_name, member, current_type)
        && field_access_is_allowed(receiver, symbol, symbols, env)
    {
        return Some(symbol.ty.clone());
    }
    if let Some((receiver_type_name, _)) =
        source_receiver_type(receiver, current_type, symbols, env)
        && let Some(java_ty) = symbols.external_super_field_type_for(&receiver_type_name, member)
    {
        return Some(java_ty);
    }
    if let Some(java_ty) =
        collection_element_static_constant_type(&receiver_ty, member, external_facts)
    {
        return Some(java_ty);
    }
    if let Some(java_ty) =
        known_external_or_pack_field_type_for_receiver(&receiver_ty, member, external_facts)
    {
        return Some(java_ty);
    }
    if let Expr::Var(receiver_name) = receiver
        && env.local_type(receiver_name).is_none()
        && env.field_type(receiver_name).is_none()
    {
        return crate::stdlib::conservative_external_enum_field_type_for_type_qualifier(
            receiver_name,
            member,
        );
    }
    None
}

fn collection_element_static_constant_type(
    receiver_ty: &JavaType,
    member: &str,
    external_facts: &FactSet,
) -> Option<JavaType> {
    let JavaType::Named { name, args } = receiver_ty else {
        return None;
    };
    if !is_collection_like_type_name(name) {
        return None;
    }
    let element_ty = args.first()?;
    known_external_or_pack_field_type_for_receiver(element_ty, member, external_facts)
}

fn known_external_or_pack_field_type_for_receiver(
    receiver_ty: &JavaType,
    member: &str,
    external_facts: &FactSet,
) -> Option<JavaType> {
    if let Some(receiver_name) = crate::stdlib::external_named_receiver_owner_key(receiver_ty)
        && let Some(field) = external_facts.external_field_for(&receiver_name, member)
        && let Some(field_type) = field.field_type.clone()
    {
        return Some(field_type);
    }
    if let Some(receiver_name) = crate::stdlib::external_named_receiver_owner_key(receiver_ty)
        && let Some(field) = crate::external_packs::external_field_for(&receiver_name, member)
        && let Some(field_type) = field.field_type
    {
        return Some(field_type);
    }
    crate::stdlib::known_external_field_type_for_receiver(receiver_ty, member)
}

fn external_fact_method_return_type_for_call(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_types: &[JavaType],
    external_facts: &FactSet,
) -> Option<JavaType> {
    let receiver_name = receiver_type.and_then(crate::stdlib::external_named_receiver_owner_key);
    external_facts
        .external_api_effect_for_receiver_with_arity(
            receiver_name.as_deref(),
            name,
            Some(arg_types.len()),
        )
        .and_then(|effect| effect.return_type.clone())
}

fn is_collection_like_type_name(name: &str) -> bool {
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "Collection"
            | "List"
            | "Set"
            | "SortedSet"
            | "NavigableSet"
            | "EnumSet"
            | "ArrayList"
            | "HashSet"
            | "LinkedHashSet"
            | "TreeSet"
    )
}

fn resolve_same_class_call(
    name: &str,
    arg_types: &[JavaType],
    methods: &[MethodSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    receiver_actual_ty: Option<&JavaType>,
) -> Result<Option<MethodResolution>, String> {
    let candidates = methods
        .iter()
        .filter(|method| {
            method.java_name == name && params_accept_arg_count(&method.params, arg_types.len())
        })
        .collect::<Vec<_>>();
    let candidates =
        deduplicated_effective_method_candidates(candidates, receiver_actual_ty, symbols);
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|method| {
            let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
            fixed_arity_exact_match(&method.params, arg_types, &substitutions)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|method| {
            let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
            callable_assignable_score(
                &method.params,
                arg_types,
                symbols,
                external_facts,
                Some(&substitutions),
            )
            .is_some()
        })
        .collect::<Vec<_>>();
    let best_assignable = best_method_match(
        &candidates,
        arg_types,
        symbols,
        external_facts,
        receiver_actual_ty,
    );

    let selected = if let [one] = exact.as_slice() {
        *one
    } else if !exact.is_empty() {
        return Err(format!(
            "ambiguous overload of {name} for argument types {:?}",
            arg_types
        ));
    } else if let Some(one) = best_assignable {
        one
    } else if assignable.len() == 1 {
        assignable[0]
    } else if assignable.is_empty() && candidates.len() == 1 {
        candidates[0]
    } else if assignable.is_empty() {
        return Err(format!(
            "no overload of {name} matches argument types {:?}",
            arg_types
        ));
    } else {
        return Err(format!(
            "ambiguous overload of {name} for argument types {:?}",
            arg_types
        ));
    };

    let mut substitutions = method_type_substitutions(selected, arg_types, symbols);
    substitutions.extend(receiver_type_substitutions(
        selected,
        receiver_actual_ty,
        symbols,
    ));
    let substituted_params: Vec<Param> = selected
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(&param.ty, &substitutions),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect();
    let param_types = substituted_params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    let arg_coercions = java_coercion_evidence_for_params(
        &selected.params,
        &param_types,
        arg_types,
        symbols,
        external_facts,
    );
    Ok(Some(MethodResolution {
        call: ResolvedCall {
            declaring_type: selected.declaring_type.clone(),
            rust_name: selected.rust_name.clone(),
            kind: if selected.is_static {
                CallKind::Static
            } else {
                CallKind::Instance
            },
            return_type: substitute_method_type_variables(&selected.return_type, &substitutions),
            arg_target_types: param_types,
            arg_coercions,
        },
        params: substituted_params,
    }))
}

fn deduplicated_effective_method_candidates<'a>(
    candidates: Vec<&'a MethodSymbol>,
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
) -> Vec<&'a MethodSymbol> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|method| {
            let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
            let substituted = substitute_method_symbol(method, &substitutions);
            seen.insert(method_signature_key(&substituted))
        })
        .collect()
}

fn method_type_substitutions(
    method: &MethodSymbol,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let mut substitutions = HashMap::new();
    for (param, arg_ty) in method.params.iter().zip(arg_types) {
        collect_method_type_substitutions(
            &param.ty,
            arg_ty,
            &mut substitutions,
            symbols,
            Some(&method.method_type_params),
        );
    }
    substitutions
}

fn receiver_type_substitutions(
    method: &MethodSymbol,
    receiver_actual_ty: Option<&JavaType>,
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let Some(JavaType::Named { name, args }) = receiver_actual_ty else {
        return HashMap::new();
    };
    let receiver_bounds = symbols.type_param_bounds_for(name).unwrap_or(&[]);
    method
        .owner_type_params
        .iter()
        .zip(args.iter().enumerate())
        .map(|(param_name, (index, arg))| {
            (
                param_name.clone(),
                wildcard_actual_or_declared_bound(arg, receiver_bounds.get(index)),
            )
        })
        .collect()
}

fn wildcard_actual_or_declared_bound(
    arg: &JavaType,
    declared_bound: Option<&Option<JavaType>>,
) -> JavaType {
    match arg {
        JavaType::Wildcard(None) => declared_bound
            .and_then(|bound| bound.clone())
            .unwrap_or_else(|| arg.clone()),
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
            bound.as_ref().clone()
        }
        _ => arg.clone(),
    }
}

fn collect_method_type_substitutions(
    param_ty: &JavaType,
    arg_ty: &JavaType,
    substitutions: &mut HashMap<String, JavaType>,
    symbols: &ProgramSymbols,
    method_type_params: Option<&[String]>,
) {
    match (param_ty, arg_ty) {
        (JavaType::Named { name, args }, actual)
            if args.is_empty() && is_method_type_variable(name, method_type_params) =>
        {
            substitutions
                .entry(name.clone())
                .or_insert_with(|| actual.clone());
        }
        (JavaType::Array(param_inner), JavaType::Array(arg_inner)) => {
            collect_method_type_substitutions(
                param_inner,
                arg_inner,
                substitutions,
                symbols,
                method_type_params,
            );
        }
        (
            JavaType::Named {
                name: param_name,
                args: param_args,
                ..
            },
            JavaType::Named { args: arg_args, .. },
        ) => {
            let projected_arg = if param_args.len() == 1 {
                symbols
                    .source_single_arg_supertype_arg(arg_ty, param_name)
                    .map(|arg| vec![arg])
            } else {
                None
            };
            let effective_arg_args = projected_arg.as_deref().unwrap_or(arg_args);
            for (param_arg, arg_arg) in param_args.iter().zip(effective_arg_args) {
                collect_method_type_substitutions(
                    param_arg,
                    arg_arg,
                    substitutions,
                    symbols,
                    method_type_params,
                );
            }
        }
        _ => {}
    }
}

fn substitute_method_type_variables(
    ty: &JavaType,
    substitutions: &HashMap<String, JavaType>,
) -> JavaType {
    match ty {
        JavaType::Named { name, args } if args.is_empty() && substitutions.contains_key(name) => {
            substitutions.get(name).cloned().unwrap()
        }
        JavaType::Array(inner) => JavaType::Array(Box::new(substitute_method_type_variables(
            inner,
            substitutions,
        ))),
        JavaType::Named { name, args } => JavaType::Named {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_method_type_variables(arg, substitutions))
                .collect(),
        },
        JavaType::Wildcard(Some(bound)) => JavaType::Wildcard(Some(match bound {
            crate::java_ir::WildcardBound::Extends(inner) => {
                crate::java_ir::WildcardBound::Extends(Box::new(substitute_method_type_variables(
                    inner,
                    substitutions,
                )))
            }
            crate::java_ir::WildcardBound::Super(inner) => crate::java_ir::WildcardBound::Super(
                Box::new(substitute_method_type_variables(inner, substitutions)),
            ),
        })),
        other => other.clone(),
    }
}

fn resolve_same_class_constructor(
    arg_types: &[JavaType],
    constructors: &[ConstructorSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Result<Option<ResolvedConstructor>, String> {
    let mut seen_candidate_signatures = HashSet::new();
    let candidates = constructors
        .iter()
        .filter(|constructor| params_accept_arg_count(&constructor.params, arg_types.len()))
        .filter(|constructor| {
            seen_candidate_signatures.insert(constructor_signature_key(constructor))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            let substitutions = constructor_type_substitutions(constructor, arg_types, symbols);
            fixed_arity_exact_match(&constructor.params, arg_types, &substitutions)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|constructor| {
            let substitutions = constructor_type_substitutions(constructor, arg_types, symbols);
            callable_assignable_score(
                &constructor.params,
                arg_types,
                symbols,
                external_facts,
                Some(&substitutions),
            )
            .is_some()
        })
        .collect::<Vec<_>>();
    let best_assignable = best_constructor_match(&candidates, arg_types, symbols, external_facts);

    let selected = if let [one] = exact.as_slice() {
        *one
    } else if !exact.is_empty() {
        return Err(format!(
            "ambiguous constructor for argument types {:?}",
            arg_types
        ));
    } else if let Some(one) = best_assignable {
        one
    } else if assignable.len() == 1 {
        assignable[0]
    } else if assignable.is_empty() && candidates.len() == 1 {
        candidates[0]
    } else if assignable.is_empty() {
        return Err(format!(
            "no constructor matches argument types {:?}",
            arg_types
        ));
    } else {
        return Err(format!(
            "ambiguous constructor for argument types {:?}",
            arg_types
        ));
    };

    let substitutions = constructor_type_substitutions(selected, arg_types, symbols);
    let params = selected
        .params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            source_name: param.source_name.clone(),
            ty: substitute_method_type_variables(&param.ty, &substitutions),
            is_varargs: param.is_varargs,
            binding: param.binding.clone(),
        })
        .collect::<Vec<_>>();
    let param_types = params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    let arg_coercions = java_coercion_evidence_for_params(
        &selected.params,
        &param_types,
        arg_types,
        symbols,
        external_facts,
    );

    Ok(Some(ResolvedConstructor {
        declaring_type: selected.declaring_type.clone(),
        rust_name: selected.rust_name.clone(),
        constructed_type: JavaType::Named {
            name: selected.declaring_type.clone(),
            args: Vec::new(),
        },
        params: selected.params.clone(),
        arg_coercions,
    }))
}

fn resolve_anonymous_class_constructor(
    target_name: &str,
    arg_types: &[JavaType],
    constructors: &[ConstructorSymbol],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Result<Option<ResolvedConstructor>, String> {
    match symbols.source_type_kind(target_name) {
        Some(TypeKind::Interface) if arg_types.is_empty() => Ok(Some(ResolvedConstructor {
            declaring_type: symbols.source_type_name(target_name).to_string(),
            rust_name: "anonymous".to_string(),
            constructed_type: JavaType::Named {
                name: symbols.source_type_name(target_name).to_string(),
                args: Vec::new(),
            },
            params: Vec::new(),
            arg_coercions: Vec::new(),
        })),
        Some(TypeKind::Class | TypeKind::Record) => {
            resolve_same_class_constructor(arg_types, constructors, symbols, external_facts)
        }
        Some(TypeKind::Enum) | None => Ok(None),
        Some(TypeKind::Interface) => Ok(None),
    }
}

fn constructor_signature_key(constructor: &ConstructorSymbol) -> String {
    let params = constructor
        .params
        .iter()
        .map(|param| format!("{:?}:{}", param.ty, param.is_varargs))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}::{}({})",
        constructor.declaring_type, constructor.rust_name, params
    )
}

fn constructor_type_substitutions(
    constructor: &ConstructorSymbol,
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
) -> HashMap<String, JavaType> {
    let mut substitutions = HashMap::new();
    for (param, arg_ty) in constructor.params.iter().zip(arg_types) {
        collect_method_type_substitutions(&param.ty, arg_ty, &mut substitutions, symbols, None);
    }
    substitutions
}

fn best_method_match<'a>(
    candidates: &[&'a MethodSymbol],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    receiver_actual_ty: Option<&JavaType>,
) -> Option<&'a MethodSymbol> {
    let mut scored = candidates
        .iter()
        .filter_map(|method| {
            let substitutions = receiver_type_substitutions(method, receiver_actual_ty, symbols);
            let param_types = method
                .params
                .iter()
                .map(|param| substitute_method_type_variables(&param.ty, &substitutions))
                .collect::<Vec<_>>();
            callable_match_score(
                &method.params,
                &param_types,
                arg_types,
                symbols,
                external_facts,
            )
            .map(|score| (*method, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(_, score)| *score);
    match scored.as_slice() {
        [(method, _)] => Some(*method),
        [(first, first_score), (_, second_score), ..] if first_score < second_score => Some(*first),
        _ => None,
    }
}

fn best_constructor_match<'a>(
    candidates: &[&'a ConstructorSymbol],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<&'a ConstructorSymbol> {
    let mut scored = candidates
        .iter()
        .filter_map(|constructor| {
            let substitutions = constructor_type_substitutions(constructor, arg_types, symbols);
            callable_assignable_score(
                &constructor.params,
                arg_types,
                symbols,
                external_facts,
                Some(&substitutions),
            )
            .map(|score| (*constructor, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(_, score)| *score);
    match scored.as_slice() {
        [(constructor, _)] => Some(*constructor),
        [(first, first_score), (_, second_score), ..] if first_score < second_score => Some(*first),
        _ => None,
    }
}

fn params_accept_arg_count(params: &[Param], arg_count: usize) -> bool {
    if params.last().is_some_and(|param| param.is_varargs) {
        arg_count >= params.len().saturating_sub(1)
    } else {
        params.len() == arg_count
    }
}

fn fixed_arity_exact_match(
    params: &[Param],
    arg_types: &[JavaType],
    substitutions: &HashMap<String, JavaType>,
) -> bool {
    params.len() == arg_types.len()
        && params.iter().zip(arg_types).all(|(param, arg_ty)| {
            arg_ty == &substitute_method_type_variables(&param.ty, substitutions)
        })
}

fn callable_assignable_score(
    params: &[Param],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    substitutions: Option<&HashMap<String, JavaType>>,
) -> Option<u32> {
    let param_types = params
        .iter()
        .map(|param| {
            substitutions
                .map(|substitutions| substitute_method_type_variables(&param.ty, substitutions))
                .unwrap_or_else(|| param.ty.clone())
        })
        .collect::<Vec<_>>();
    callable_match_score(params, &param_types, arg_types, symbols, external_facts)
}

fn callable_match_score(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if !params.last().is_some_and(|param| param.is_varargs) {
        return overload_match_score(
            &param_types.iter().collect::<Vec<_>>(),
            arg_types,
            symbols,
            external_facts,
        );
    }

    if arg_types.len() < params.len().saturating_sub(1) {
        return None;
    }

    if params.len() == arg_types.len() {
        let fixed_score = overload_match_score(
            &param_types.iter().collect::<Vec<_>>(),
            arg_types,
            symbols,
            external_facts,
        );
        if fixed_score.is_some() {
            return fixed_score;
        }
    }

    let fixed_count = params.len().saturating_sub(1);
    let fixed_score = overload_match_score(
        &param_types[..fixed_count].iter().collect::<Vec<_>>(),
        &arg_types[..fixed_count],
        symbols,
        external_facts,
    )?;
    let JavaType::Array(vararg_element) = &param_types[fixed_count] else {
        return None;
    };
    let expanded_score = arg_types[fixed_count..]
        .iter()
        .map(|arg_ty| java_type_match_score(arg_ty, vararg_element, symbols, external_facts))
        .sum::<Option<u32>>()?;

    Some(fixed_score + expanded_score + 5)
}

pub fn overload_applicability_report(
    params: &[Param],
    arg_types: &[JavaType],
    source_symbols: &SourceSymbols,
) -> OverloadApplicabilityReport {
    let external_facts = FactSet::default();
    let param_types = params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    callable_applicability_report(
        params,
        &param_types,
        arg_types,
        &source_symbols.symbols,
        &external_facts,
    )
}

fn callable_applicability_report(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> OverloadApplicabilityReport {
    if !params.last().is_some_and(|param| param.is_varargs) {
        return fixed_arity_applicability_report(param_types, arg_types, symbols, external_facts);
    }

    if arg_types.len() < params.len().saturating_sub(1) {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::ArityMismatch {
                expected: params.len().saturating_sub(1),
                actual: arg_types.len(),
                varargs: true,
            }],
        };
    }

    if params.len() == arg_types.len() {
        let fixed =
            fixed_arity_applicability_report(param_types, arg_types, symbols, external_facts);
        if fixed.is_applicable() {
            return fixed;
        }
    }

    let fixed_count = params.len().saturating_sub(1);
    let JavaType::Array(vararg_element) = &param_types[fixed_count] else {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::MissingVarargsArrayParam],
        };
    };
    let mut report = fixed_arity_prefix_applicability_report(
        &param_types[..fixed_count],
        &arg_types[..fixed_count],
        symbols,
        external_facts,
        OverloadApplicabilityMode::VariableArity,
    );
    if !report.rejections.is_empty() {
        return report;
    }
    for (offset, arg_ty) in arg_types[fixed_count..].iter().enumerate() {
        let arg_index = fixed_count + offset;
        push_argument_applicability(
            &mut report,
            arg_index,
            arg_ty,
            vararg_element,
            symbols,
            external_facts,
        );
    }
    if report.rejections.is_empty() {
        let total = report.arguments.iter().map(|arg| arg.score).sum::<u32>() + 5;
        report.score = Some(total);
    }

    report
}

fn java_coercion_evidence_for_params(
    params: &[Param],
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Vec<JavaCoercionEvidence> {
    callable_applicability_report(params, param_types, arg_types, symbols, external_facts)
        .arguments
        .into_iter()
        .map(|argument| JavaCoercionEvidence {
            arg_index: argument.arg_index,
            actual: argument.actual,
            target: argument.target,
            kind: java_coercion_kind_from_overload_conversion(argument.conversion),
        })
        .collect()
}

fn java_coercion_kind_from_overload_conversion(
    conversion: OverloadArgumentConversion,
) -> JavaCoercionKind {
    match conversion {
        OverloadArgumentConversion::Exact => JavaCoercionKind::Identity,
        OverloadArgumentConversion::PrimitiveWidening => JavaCoercionKind::PrimitiveWidening,
        OverloadArgumentConversion::Boxing => JavaCoercionKind::Boxing,
        OverloadArgumentConversion::Unboxing => JavaCoercionKind::Unboxing,
        OverloadArgumentConversion::BoxingWidening => JavaCoercionKind::BoxingWidening,
        OverloadArgumentConversion::ReferenceWidening => JavaCoercionKind::ReferenceWidening,
        OverloadArgumentConversion::GenericInference => JavaCoercionKind::GenericInference,
        OverloadArgumentConversion::NullToReference => JavaCoercionKind::NullToReference,
        OverloadArgumentConversion::LambdaToFunctionalInterface => {
            JavaCoercionKind::LambdaAdaptation
        }
        OverloadArgumentConversion::MethodReferenceToFunctionalInterface => {
            JavaCoercionKind::MethodReferenceAdaptation
        }
        OverloadArgumentConversion::UnknownFallback => JavaCoercionKind::UnknownFallback,
    }
}

fn overload_match_score(
    params: &[&JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if params.len() != arg_types.len() {
        return None;
    }
    params
        .iter()
        .zip(arg_types)
        .map(|(param, arg_ty)| java_type_match_score(arg_ty, param, symbols, external_facts))
        .sum()
}

fn java_type_applicability(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Result<(OverloadArgumentConversion, u32), OverloadRejectionReason> {
    let actual_java = actual.clone();
    let target_java = target.clone();
    if actual == target {
        return Ok((OverloadArgumentConversion::Exact, 0));
    }
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named { name: target, .. },
        ) if actual_args.is_empty() && is_method_type_variable(actual_name, None) => {
            let score = if crate::stdlib::is_known_structural_reference_target(target) {
                45
            } else {
                55
            };
            Ok((OverloadArgumentConversion::GenericInference, score))
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Ok((
                OverloadArgumentConversion::GenericInference,
                if matches!(actual, JavaType::Primitive(_)) {
                    65
                } else {
                    40
                },
            ))
        }
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_widening_distance(*actual, *target)
                .map(|distance| {
                    (
                        OverloadArgumentConversion::PrimitiveWidening,
                        1 + u32::from(distance),
                    )
                })
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Ok((OverloadArgumentConversion::BoxingWidening, 70))
            } else if target == "Number" {
                primitive_can_box_to_number(*actual)
                    .then_some((OverloadArgumentConversion::BoxingWidening, 65))
                    .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
            } else {
                crate::stdlib::boxed_primitive_type(target)
                    .and_then(|target| {
                        primitive_widening_distance(*actual, target).map(|distance| {
                            (
                                if distance == 0 {
                                    OverloadArgumentConversion::Boxing
                                } else {
                                    OverloadArgumentConversion::BoxingWidening
                                },
                                2 + u32::from(distance),
                            )
                        })
                    })
                    .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual)
                .and_then(|actual| {
                    primitive_widening_distance(actual, *target).map(|distance| {
                        (
                            OverloadArgumentConversion::Unboxing,
                            2 + u32::from(distance),
                        )
                    })
                })
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Named { .. }, JavaType::Named { name, .. }) if name == "Object" => {
            Ok((OverloadArgumentConversion::ReferenceWidening, 60))
        }
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => {
            Ok((OverloadArgumentConversion::ReferenceWidening, 60))
        }
        (JavaType::String, JavaType::Named { name, .. }) => {
            crate::stdlib::is_known_java_subtype("String", name)
                .then_some((OverloadArgumentConversion::ReferenceWidening, 10))
                .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java))
        }
        (
            JavaType::Named {
                name: actual,
                args: actual_args,
            },
            JavaType::Named {
                name: target,
                args: target_args,
            },
        ) => named_reference_match_score(
            actual,
            actual_args,
            target,
            target_args,
            symbols,
            external_facts,
        )
        .map(|score| (OverloadArgumentConversion::ReferenceWidening, score))
        .ok_or_else(|| argument_type_mismatch(&actual_java, &target_java)),
        (JavaType::Unknown(name), _) if name == "null" => {
            if matches!(target, JavaType::Primitive(_)) {
                Err(OverloadRejectionReason::NullToPrimitive {
                    arg_index: 0,
                    target: target_java.clone(),
                })
            } else {
                Ok((OverloadArgumentConversion::NullToReference, 30))
            }
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. }) if name == "lambda" => {
            symbols
                .is_functional_interface_type(target)
                .then_some((OverloadArgumentConversion::LambdaToFunctionalInterface, 20))
                .ok_or_else(
                    || OverloadRejectionReason::LambdaTargetNotFunctionalInterface {
                        arg_index: 0,
                        target: target_java.clone(),
                    },
                )
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if name == "method_reference" =>
        {
            symbols
                .is_functional_interface_type(target)
                .then_some((
                    OverloadArgumentConversion::MethodReferenceToFunctionalInterface,
                    20,
                ))
                .ok_or_else(|| {
                    OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                        arg_index: 0,
                        target: target_java.clone(),
                    }
                })
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if unknown_collection_factory_matches_target(name, target) =>
        {
            Ok((OverloadArgumentConversion::UnknownFallback, 25))
        }
        (JavaType::Unknown(_), JavaType::Void) => {
            Err(argument_type_mismatch(&actual_java, &target_java))
        }
        (JavaType::Unknown(_), JavaType::Primitive(_)) => {
            Ok((OverloadArgumentConversion::UnknownFallback, 95))
        }
        (JavaType::Unknown(_), _) => Ok((OverloadArgumentConversion::UnknownFallback, 80)),
        _ => Err(argument_type_mismatch(&actual_java, &target_java)),
    }
}

fn argument_type_mismatch(actual: &JavaType, target: &JavaType) -> OverloadRejectionReason {
    OverloadRejectionReason::ArgumentTypeMismatch {
        arg_index: 0,
        actual: actual.clone(),
        target: target.clone(),
    }
}

fn fixed_arity_applicability_report(
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> OverloadApplicabilityReport {
    fixed_arity_prefix_applicability_report(
        param_types,
        arg_types,
        symbols,
        external_facts,
        OverloadApplicabilityMode::FixedArity,
    )
}

fn fixed_arity_prefix_applicability_report(
    param_types: &[JavaType],
    arg_types: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
    mode: OverloadApplicabilityMode,
) -> OverloadApplicabilityReport {
    if param_types.len() != arg_types.len() {
        return OverloadApplicabilityReport {
            mode: None,
            score: None,
            arguments: Vec::new(),
            rejections: vec![OverloadRejectionReason::ArityMismatch {
                expected: param_types.len(),
                actual: arg_types.len(),
                varargs: mode == OverloadApplicabilityMode::VariableArity,
            }],
        };
    }
    let mut report = OverloadApplicabilityReport {
        mode: Some(mode),
        score: None,
        arguments: Vec::new(),
        rejections: Vec::new(),
    };
    for (arg_index, (target, actual)) in param_types.iter().zip(arg_types).enumerate() {
        push_argument_applicability(
            &mut report,
            arg_index,
            actual,
            target,
            symbols,
            external_facts,
        );
    }
    if report.rejections.is_empty() {
        report.score = Some(report.arguments.iter().map(|arg| arg.score).sum());
    }
    report
}

fn push_argument_applicability(
    report: &mut OverloadApplicabilityReport,
    arg_index: usize,
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) {
    match java_type_applicability(actual, target, symbols, external_facts) {
        Ok((conversion, score)) => report.arguments.push(OverloadArgumentApplicability {
            arg_index,
            actual: actual.clone(),
            target: target.clone(),
            conversion,
            score,
        }),
        Err(reason) => report.rejections.push(reason.with_arg_index(arg_index)),
    }
}

impl OverloadRejectionReason {
    fn with_arg_index(self, arg_index: usize) -> Self {
        match self {
            OverloadRejectionReason::ArgumentTypeMismatch { actual, target, .. } => {
                OverloadRejectionReason::ArgumentTypeMismatch {
                    arg_index,
                    actual,
                    target,
                }
            }
            OverloadRejectionReason::NullToPrimitive { target, .. } => {
                OverloadRejectionReason::NullToPrimitive { arg_index, target }
            }
            OverloadRejectionReason::LambdaTargetNotFunctionalInterface { target, .. } => {
                OverloadRejectionReason::LambdaTargetNotFunctionalInterface { arg_index, target }
            }
            OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                target, ..
            } => OverloadRejectionReason::MethodReferenceTargetNotFunctionalInterface {
                arg_index,
                target,
            },
            other => other,
        }
    }
}

fn java_type_match_score(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return Some(0);
    }
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named { name: target, .. },
        ) if actual_args.is_empty() && is_method_type_variable(actual_name, None) => {
            if crate::stdlib::is_known_structural_reference_target(target) {
                Some(45)
            } else {
                Some(55)
            }
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Some(if matches!(actual, JavaType::Primitive(_)) {
                65
            } else {
                40
            })
        }
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_widening_distance(*actual, *target).map(|distance| 1 + u32::from(distance))
        }
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Some(70)
            } else if target == "Number" {
                primitive_can_box_to_number(*actual).then_some(65)
            } else {
                crate::stdlib::boxed_primitive_type(target).and_then(|target| {
                    primitive_widening_distance(*actual, target)
                        .map(|distance| 2 + u32::from(distance))
                })
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual).and_then(|actual| {
                primitive_widening_distance(actual, *target).map(|distance| 2 + u32::from(distance))
            })
        }
        (JavaType::Named { .. }, JavaType::Named { name, .. }) if name == "Object" => Some(60),
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => Some(60),
        (JavaType::String, JavaType::Named { name, .. }) => {
            crate::stdlib::is_known_java_subtype("String", name).then_some(10)
        }
        (
            JavaType::Named {
                name: actual,
                args: actual_args,
            },
            JavaType::Named {
                name: target,
                args: target_args,
            },
        ) => named_reference_match_score(
            actual,
            actual_args,
            target,
            target_args,
            symbols,
            external_facts,
        ),
        (JavaType::Unknown(name), _) if name == "null" => {
            (!matches!(target, JavaType::Primitive(_))).then_some(30)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. }) if name == "lambda" => {
            symbols.is_functional_interface_type(target).then_some(20)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if name == "method_reference" =>
        {
            symbols.is_functional_interface_type(target).then_some(20)
        }
        (JavaType::Unknown(name), JavaType::Named { name: target, .. })
            if unknown_collection_factory_matches_target(name, target) =>
        {
            Some(25)
        }
        (JavaType::Unknown(_), JavaType::Void) => None,
        (JavaType::Unknown(_), JavaType::Primitive(_)) => Some(95),
        (JavaType::Unknown(_), _) => Some(80),
        _ => None,
    }
}

fn unknown_collection_factory_matches_target(name: &str, target: &str) -> bool {
    matches!(
        (name, target),
        ("emptyList", "List" | "Collection" | "Iterable")
            | ("emptySet", "Set" | "Collection" | "Iterable")
            | ("emptyMap", "Map")
    )
}

fn java_type_assignable(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> bool {
    java_type_match_score(actual, target, symbols, external_facts).is_some()
}

fn named_reference_match_score(
    actual: &str,
    actual_args: &[JavaType],
    target: &str,
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts);
    }

    if target == "Enum" && symbols.is_source_enum_type(actual) {
        return generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .or_else(|| target_args.iter().all(is_unbounded_wildcard).then_some(12));
    }

    if crate::stdlib::is_known_single_arg_generic_supertype(actual, target) {
        return generic_supertype_match_score(actual_args, target_args, symbols, external_facts)
            .map(|score| 10 + score);
    }

    if let Some(distance) = symbols.source_subtype_distance(actual, target, external_facts) {
        return Some(
            10 + distance
                + generic_arguments_match_score(actual_args, target_args, symbols, external_facts)?,
        );
    }

    crate::stdlib::is_known_java_subtype(actual, target).then(|| {
        generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
            .unwrap_or(10)
    })
}

fn generic_supertype_match_score(
    actual_args: &[JavaType],
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if target_args.is_empty() {
        return Some(0);
    }
    if actual_args.is_empty() {
        return Some(50);
    }
    generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
}

fn generic_arguments_match_score(
    actual_args: &[JavaType],
    target_args: &[JavaType],
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    match (actual_args, target_args) {
        (_, []) => Some(0),
        ([], targets) if targets.iter().all(is_unbounded_wildcard) => Some(5),
        ([], _) => Some(50),
        (actuals, targets) if actuals.len() == targets.len() => actuals
            .iter()
            .zip(targets)
            .map(|(actual, target)| {
                generic_argument_match_score(actual, target, symbols, external_facts)
            })
            .sum(),
        _ => None,
    }
}

fn generic_argument_match_score(
    actual: &JavaType,
    target: &JavaType,
    symbols: &ProgramSymbols,
    external_facts: &FactSet,
) -> Option<u32> {
    if actual == target {
        return Some(0);
    }
    match (actual, target) {
        (JavaType::Primitive(actual), JavaType::Named { name: target, .. }) => {
            if target == "Object" {
                Some(70)
            } else if target == "Number" {
                primitive_can_box_to_number(*actual).then_some(65)
            } else {
                crate::stdlib::boxed_primitive_type(target).and_then(|target| {
                    primitive_widening_distance(*actual, target)
                        .map(|distance| 2 + u32::from(distance))
                })
            }
        }
        (JavaType::Named { name: actual, .. }, JavaType::Primitive(target)) => {
            crate::stdlib::boxed_primitive_type(actual).and_then(|actual| {
                primitive_widening_distance(actual, *target).map(|distance| 2 + u32::from(distance))
            })
        }
        (JavaType::Array(actual_inner), JavaType::Array(target_inner)) => {
            generic_argument_match_score(actual_inner, target_inner, symbols, external_facts)
                .map(|score| score + 1)
        }
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) if actual_name == target_name => {
            generic_arguments_match_score(actual_args, target_args, symbols, external_facts)
                .map(|score| score + 1)
        }
        (_, JavaType::Named { name, args })
            if args.is_empty() && is_method_type_variable(name, None) =>
        {
            Some(if matches!(actual, JavaType::Primitive(_)) {
                45
            } else {
                20
            })
        }
        (_, JavaType::Wildcard(None)) => Some(5),
        (_, JavaType::Wildcard(Some(WildcardBound::Extends(bound)))) => {
            java_type_match_score(actual, bound, symbols, external_facts).map(|score| 5 + score)
        }
        (_, JavaType::Wildcard(Some(WildcardBound::Super(bound)))) => {
            java_type_match_score(bound, actual, symbols, external_facts).map(|score| 5 + score)
        }
        _ => None,
    }
}

fn is_unbounded_wildcard(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Wildcard(None))
}

fn is_method_type_variable(name: &str, declared_method_type_params: Option<&[String]>) -> bool {
    if let Some(declared) = declared_method_type_params {
        return declared.iter().any(|param| param == name);
    }
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_uppercase())
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

/// How far `actual` must widen to reach `target`, for ranking overloads.
///
/// The rank-subtraction form this replaces put `char` beside `short` on one
/// chain, so `char`→`short` and `short`→`char` both scored **distance 0** —
/// an exact match — and `f(short)` tied with `f(char)` for a `char` argument.
/// Neither conversion exists in Java without a cast.
fn primitive_widening_distance(
    actual: crate::java_ir::PrimitiveType,
    target: crate::java_ir::PrimitiveType,
) -> Option<u8> {
    crate::types::primitive_widening_steps(actual, target)
}

fn primitive_can_box_to_number(primitive: crate::java_ir::PrimitiveType) -> bool {
    use crate::java_ir::PrimitiveType::*;
    matches!(primitive, Byte | Short | Int | Long | Float | Double)
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
        collect_type_capability_facts_for_type(ty, facts, &[]);
    }
}

fn collect_field_declaration_facts(program: &Program, facts: &mut FactSet) {
    for ty in &program.types {
        collect_field_declaration_facts_for_type(ty, facts, program.package.as_deref(), None);
    }
}

fn collect_field_declaration_facts_for_type(
    ty: &TypeDecl,
    facts: &mut FactSet,
    package: Option<&str>,
    outer_name: Option<&str>,
) {
    let type_name = package_qualified_source_type_name(ty, package, outer_name);
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                let is_static_final = field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final);
                facts.external_field_facts.push(ExternalFieldFact {
                    field: FieldKey {
                        type_name: type_name.clone(),
                        field_name: field.name.clone(),
                    },
                    field_type: Some(field.ty.clone()),
                    field_kind: if is_static_final {
                        ExternalFieldKind::StaticFinal
                    } else {
                        ExternalFieldKind::Unknown
                    },
                    provenance: FactProvenance::ParsedSource,
                });
            }
            Member::Type(nested) => {
                collect_field_declaration_facts_for_type(nested, facts, package, Some(&type_name))
            }
            _ => {}
        }
    }
}

/// The declared values of every `static final` field that is a Java constant
/// expression.
///
/// Without this the value is simply dropped and the facade emits a
/// `compile_error!` for a constant whose initialiser the translation just
/// parsed — group 3 of
/// [0031](../claude_audits/done/0031-stubbed-constant-census.md). Anything that does
/// not fold is left alone, so the diagnostic still stands wherever the value
/// really is unknown.
///
/// Run over the whole program rather than per type, because a constant may be
/// defined in terms of one owned by another class
/// (`VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR`). That is a
/// dependency graph, so it is iterated to a fixpoint instead of assuming
/// declaration order resolves it.
fn collect_static_final_constant_value_facts(program: &Program, facts: &mut FactSet) {
    let declarations = static_final_constant_declarations(program);
    facts.extend_from(constant_value_facts_from_declarations(&declarations));
}

/// Every `static final` field of `program`, paired with its package-qualified
/// owner.
///
/// Exposed so a caller holding several parsed files can pool them: type names
/// are qualified with each file's *own* package, which a merged program no
/// longer knows.
pub fn static_final_constant_declarations(program: &Program) -> Vec<ConstantDeclaration> {
    let mut declarations = Vec::new();
    for ty in &program.types {
        collect_static_final_constant_declarations(
            ty,
            program.package.as_deref(),
            None,
            &mut declarations,
        );
    }
    declarations
}

/// One `static final` declaration, with what a fold needs to resolve it.
///
/// The package is carried separately rather than recovered from `owner`,
/// because a nested type's owner is `pkg.Outer.Inner` and splitting it would
/// give `pkg.Outer` as the package. Getting that wrong is how a simple-name
/// reference resolves into the wrong package
/// ([0031](../claude_audits/done/0031-stubbed-constant-census.md)).
#[derive(Debug, Clone)]
pub struct ConstantDeclaration {
    /// Package-qualified owner, e.g. `pkg1.Constants` or `pkg1.Outer.Inner`.
    pub owner: String,
    /// The declaring file's package, if any.
    pub package: Option<String>,
    pub field: crate::java_ir::FieldDecl,
}

/// Folds `declarations` to a fixpoint and reports the values that resolve.
pub fn constant_value_facts_from_declarations(declarations: &[ConstantDeclaration]) -> FactSet {
    let (facts, _) = fold_constant_declarations(declarations);
    facts
}

/// Folds `declarations` and returns both the value facts and the scope they
/// were folded in.
///
/// The scope is what a *later* fold needs in order to resolve a constant owned
/// by another type — `static int[] a = new int[Other.N];`, which is ordinary
/// Java and which a per-owner scope cannot answer
/// ([0039](../claude_audits/done/0039-static-array-fields-lose-their-value-and-their-writes.md)).
pub fn fold_constant_declarations(
    declarations: &[ConstantDeclaration],
) -> (FactSet, crate::source_constant_values::ConstantScope) {
    let mut facts = FactSet::default();
    let mut scope = crate::source_constant_values::ConstantScope::new();
    let mut folded: std::collections::BTreeMap<
        usize,
        crate::source_constant_values::ConstantValue,
    > = std::collections::BTreeMap::new();
    // Each round can only resolve references to constants folded in an earlier
    // round, so the number of rounds is bounded by the longest chain. Stopping
    // when a round adds nothing is what makes that bound implicit.
    loop {
        let mut progressed = false;
        for (index, declaration) in declarations.iter().enumerate() {
            if folded.contains_key(&index) {
                continue;
            }
            let Some(initializer) = declaration.field.initializer.as_ref() else {
                continue;
            };
            // A type's own constants are visible to it as bare names, and must
            // not leak to any other owner — so they go into a copy of the scope
            // pointed at this declaration.
            let mut owner_scope = scope.clone();
            let mut own = std::collections::BTreeMap::new();
            for (other_index, other) in declarations.iter().enumerate() {
                if other.owner != declaration.owner {
                    continue;
                }
                let Some(value) = folded.get(&other_index) else {
                    continue;
                };
                for alias in [
                    other.field.name.clone(),
                    crate::parser::to_snake_case(&other.field.name),
                ] {
                    own.insert(alias, value.clone());
                }
            }
            owner_scope.focus_on_owner(declaration.package.as_deref(), own);

            let Some(value) =
                crate::source_constant_values::fold_constant_expr(initializer, &owner_scope)
            else {
                continue;
            };
            scope.record(&declaration.owner, &declaration.field.name, value.clone());
            folded.insert(index, value);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }

    for (index, declaration) in declarations.iter().enumerate() {
        let Some(value) = folded.get(&index) else {
            continue;
        };
        if let Some(rust_value) = value.rust_value_for(&declaration.field.ty) {
            facts
                .external_field_value_facts
                .push(crate::facts::ExternalFieldValueFact {
                    field: FieldKey {
                        type_name: declaration.owner.clone(),
                        field_name: declaration.field.name.clone(),
                    },
                    rust_value,
                    provenance: FactProvenance::ParsedSource,
                });
        }
    }
    (facts, scope)
}

fn collect_static_final_constant_declarations(
    ty: &TypeDecl,
    package: Option<&str>,
    outer_name: Option<&str>,
    into: &mut Vec<ConstantDeclaration>,
) {
    let type_name = package_qualified_source_type_name(ty, package, outer_name);
    for member in &ty.members {
        match member {
            Member::Field(field)
                if field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final) =>
            {
                into.push(ConstantDeclaration {
                    owner: type_name.clone(),
                    package: package.map(str::to_string),
                    field: field.clone(),
                });
            }
            Member::Type(nested) => {
                collect_static_final_constant_declarations(nested, package, Some(&type_name), into)
            }
            _ => {}
        }
    }
}

fn package_qualified_source_type_name(
    ty: &TypeDecl,
    package: Option<&str>,
    outer_name: Option<&str>,
) -> String {
    if ty.name.contains('.') {
        return ty.name.clone();
    }
    if let Some(outer_name) = outer_name {
        return format!("{outer_name}.{}", ty.name);
    }
    match package {
        Some(package) if !package.is_empty() => format!("{package}.{}", ty.name),
        _ => ty.name.clone(),
    }
}

fn collect_type_capability_facts_for_type(
    ty: &TypeDecl,
    facts: &mut FactSet,
    inferred_final_type_names: &[String],
) {
    let object_safety = (ty.kind == TypeKind::Interface).then(|| interface_object_safety_fact(ty));
    let inferred_closed_nested_variants = inferred_closed_nested_variants_for_type(ty);
    let inferred_final = inferred_final_type_names
        .iter()
        .any(|type_name| type_name == &ty.name);
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
                .map(|name| crate::parser::parse_hierarchy_type(name))
                .collect(),
            extends: ty
                .extends
                .as_ref()
                .map(|name| crate::parser::parse_hierarchy_type(name)),
            final_type: Some(ty.modifiers.contains(&Modifier::Final) || inferred_final),
            object_safe_trait: object_safety.as_ref().map(|fact| fact.object_safe),
            provenance: FactProvenance::ParsedSource,
        });
    if let Some(object_safety) = object_safety {
        facts.object_safety_facts.push(object_safety);
    }
    if (ty.modifiers.contains(&Modifier::Sealed) && !ty.permits.is_empty())
        || !inferred_closed_nested_variants.is_empty()
    {
        facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
            type_name: ty.name.clone(),
            closed: true,
            provenance: if inferred_closed_nested_variants.is_empty() {
                FactProvenance::ParsedSource
            } else {
                FactProvenance::CorpusAnalysis
            },
        });
    }
    if ty.modifiers.contains(&Modifier::Sealed) && !ty.permits.is_empty() {
        facts.sealed_permits_facts.push(SealedPermitsFact {
            type_name: ty.name.clone(),
            permitted_subtypes: ty.permits.clone(),
            provenance: FactProvenance::ParsedSource,
        });
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_capability_facts_for_type(nested, facts, &inferred_closed_nested_variants);
        }
    }
}

fn inferred_closed_nested_variants_for_type(ty: &TypeDecl) -> Vec<String> {
    if ty.kind != TypeKind::Class || !ty.modifiers.contains(&Modifier::Abstract) {
        return Vec::new();
    }
    let variants = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Type(nested)
                if nested.kind == TypeKind::Class
                    && !nested.modifiers.contains(&Modifier::Abstract)
                    && nested_extends_type(nested, &ty.name)
                    && constructors_are_not_publicly_extensible(nested) =>
            {
                Some(nested.name.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if variants.is_empty() {
        return Vec::new();
    }
    let all_direct_nested_subclasses_are_variants = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Type(nested) if nested_extends_type(nested, &ty.name) => Some(nested),
            _ => None,
        })
        .all(|nested| variants.iter().any(|variant| variant == &nested.name));
    all_direct_nested_subclasses_are_variants
        .then_some(variants)
        .unwrap_or_default()
}

fn nested_extends_type(ty: &TypeDecl, super_name: &str) -> bool {
    ty.extends
        .as_ref()
        .map(|extends| crate::parser::parse_hierarchy_type(extends))
        .as_ref()
        .and_then(java_type_simple_name_option)
        == Some(super_name)
}

fn java_type_simple_name_option(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(name.rsplit('.').next().unwrap_or(name))
        }
        JavaType::String => Some("String"),
        _ => None,
    }
}

fn constructors_are_not_publicly_extensible(ty: &TypeDecl) -> bool {
    let constructors = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => Some(constructor),
            _ => None,
        })
        .collect::<Vec<_>>();
    if constructors.is_empty() {
        return !ty.modifiers.contains(&Modifier::Public)
            && !ty.modifiers.contains(&Modifier::Protected);
    }
    constructors.iter().all(|constructor| {
        !constructor.modifiers.contains(&Modifier::Public)
            && !constructor.modifiers.contains(&Modifier::Protected)
    })
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
            Member::Type(_) => {}
            Member::Unsupported { .. } => violations.push(ObjectSafetyViolation::UnsupportedMember),
            Member::Field(_) => {}
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
        return;
    }
    let combinator_self_return = is_combinator_self_return_method(method, interface_name);
    match java_type_self_reference_kind(&method.return_type, interface_name) {
        Some(InterfaceSelfReferenceKind::Direct)
            if !is_clone_like_self_return_method(method) && !combinator_self_return =>
        {
            violations.push(ObjectSafetyViolation::SelfReferentialReturn {
                method_name: method.name.clone(),
            });
        }
        Some(InterfaceSelfReferenceKind::Direct) => {}
        Some(InterfaceSelfReferenceKind::Nested) => {
            violations.push(ObjectSafetyViolation::NestedSelfReferentialReturn {
                method_name: method.name.clone(),
            });
        }
        None => {}
    }
    for param in &method.params {
        if combinator_self_return {
            continue;
        }
        match java_type_self_reference_kind(&param.ty, interface_name) {
            Some(InterfaceSelfReferenceKind::Direct) => {}
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

fn is_clone_like_self_return_method(method: &MethodDecl) -> bool {
    method.params.is_empty() && matches!(method.name.as_str(), "copy" | "deepCopy" | "clone")
}

fn is_combinator_self_return_method(method: &MethodDecl, interface_name: &str) -> bool {
    matches!(
        method.name.as_str(),
        "and" | "or" | "negate" | "andThen" | "compose"
    ) && java_type_self_reference_kind(&method.return_type, interface_name)
        == Some(InterfaceSelfReferenceKind::Direct)
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
        let outer_fields = outer.map(instance_field_types).unwrap_or_default();
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
                // A `static` nested class has no enclosing instance, so nothing
                // in it can capture one. Analysing it as an inner class made an
                // unqualified reference to an outer *static* lower to
                // `self.__outer.borrow().field`
                // ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
                Member::Type(inner) if !inner.modifiers.contains(&Modifier::Static) => {
                    self.analyze_inner_type(inner, ty)
                }
                Member::Type(inner) => self.analyze_type_members(inner, None),
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
        let outer_fields = outer.map(instance_field_types).unwrap_or_default();
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
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
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
                ..
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
                        .unwrap_or_else(|| JavaType::Unknown(catch.source_name.clone()));
                    add_symbol(
                        &mut catch_ctx,
                        &catch.source_name,
                        ty,
                        CaptureSource::Local,
                        false,
                    );
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
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.analyze_block(block, &mut ctx.clone(), None)
            }
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
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                self.collect_block_captures(block, &mut ctx.clone(), captures)
            }
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
            static_init_blocks: Vec::new(),
            kind: crate::java_ir::TypeKind::Class,
            name: "Anonymous".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
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

/// Instance fields only.
///
/// Used for the *enclosing* type when classifying outer-`this` captures: a
/// `static` member is reached through the class, not through an instance, so
/// naming one from a nested class is not a capture at all. Including statics
/// here produced `self.__outer.borrow().counter` for a field that is not even a
/// struct member ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
fn instance_field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                Some((field.name.clone(), field.ty.clone()))
            }
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
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
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
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
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
