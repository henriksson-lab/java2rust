use crate::call_plan::{NativeCollectionNewTarget, plan_native_collection_constructor_for_args};
use crate::diagnostics::Severity;
use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, Block, CatchClause, Expr, JavaType, LambdaBody, Member, MethodDecl, Program, Stmt,
    SwitchCase, SwitchLabel, TypeDecl, TypeKind,
};
use crate::representation::RepresentationPlan;
use crate::rewrite::{MethodControlFlowPlan, control_flow_rewrite_diagnostics};
use crate::signature::SignaturePlan;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringDiagnosticPlan {
    severity: Severity,
    code: &'static str,
    message: String,
}

impl LoweringDiagnosticPlan {
    pub(crate) fn new(severity: Severity, code: &'static str, message: String) -> Self {
        Self {
            severity,
            code,
            message,
        }
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_parts(self) -> (Severity, &'static str, String) {
        (self.severity, self.code, self.message)
    }
}

pub fn collect_lowering_diagnostic_plans(
    program: &Program,
    support_local_type_program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
) -> Vec<LoweringDiagnosticPlan> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(facts.diagnostics().into_iter().map(|diagnostic| {
        let (code, message) = diagnostic.into_parts();
        LoweringDiagnosticPlan::new(Severity::Warning, code, message)
    }));
    diagnostics.extend(
        representations
            .diagnostics_for_program_with_anonymous_context(program, support_local_type_program)
            .into_iter()
            .map(|diagnostic| {
                LoweringDiagnosticPlan::new(
                    Severity::Unsupported,
                    diagnostic.code(),
                    diagnostic.message().to_string(),
                )
            }),
    );
    diagnostics.extend(signatures.diagnostics().iter().map(|diagnostic| {
        LoweringDiagnosticPlan::new(
            Severity::Warning,
            diagnostic.code(),
            diagnostic.message().to_string(),
        )
    }));
    diagnostics.extend(
        control_flow_rewrite_diagnostics(control_flow_plans)
            .into_iter()
            .map(|diagnostic| {
                LoweringDiagnosticPlan::new(
                    Severity::Unsupported,
                    diagnostic.code(),
                    diagnostic.message().to_string(),
                )
            }),
    );
    diagnostics.extend(collect_java_reference_identity_equality_diagnostics(
        program,
        facts,
        representations,
    ));
    diagnostics
}

fn collect_java_reference_identity_equality_diagnostics(
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Vec<LoweringDiagnosticPlan> {
    let identity = SourceIdentityFacts::from_program(program, facts, representations);
    let mut diagnostics = Vec::new();
    for ty in &program.types {
        collect_type_reference_identity_equality_diagnostics(
            ty,
            facts,
            &identity,
            &mut diagnostics,
        );
    }
    diagnostics
}

/// Which source types declare their own `equals`.
///
/// Java's `Object.equals` **is** reference identity unless a class overrides it,
/// so `a.equals(b)` on a class that does not override it should answer the same
/// as `a == b`. Lowering compares structurally either way, which answers `true`
/// where Java says `false` — and in the wrong direction: a `HashMap` or a
/// `contains` built on it **merges entries Java keeps apart**
/// ([0028](../claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md)).
#[derive(Debug, Default)]
struct SourceIdentityFacts {
    declared: std::collections::BTreeSet<String>,
    source_classes: std::collections::BTreeSet<String>,
    declares_equals: std::collections::BTreeSet<String>,
    extends: std::collections::BTreeMap<String, String>,
    implemented_reference_identity: std::collections::BTreeSet<String>,
    source_hierarchy_carriers: std::collections::BTreeSet<String>,
    concrete_comparator_factory_methods: std::collections::BTreeSet<(String, usize)>,
}

impl SourceIdentityFacts {
    fn from_program(
        program: &Program,
        storage_facts: &FactSet,
        representations: &RepresentationPlan,
    ) -> Self {
        let mut facts = Self::default();
        for ty in &program.types {
            facts.collect_type(ty, program, storage_facts, representations);
        }
        facts
    }

    fn collect_type(
        &mut self,
        ty: &TypeDecl,
        program: &Program,
        storage_facts: &FactSet,
        representations: &RepresentationPlan,
    ) {
        let simple = simple_type_name(&ty.name).to_string();
        self.declared.insert(simple.clone());
        if matches!(ty.kind, TypeKind::Class | TypeKind::Record) {
            self.source_classes.insert(simple.clone());
        }
        if representations
            .type_representation(&ty.name)
            .is_some_and(|entry| {
                matches!(
                    entry.representation(),
                    crate::representation::ObjectRepresentation::ClosedEnum { .. }
                        | crate::representation::ObjectRepresentation::SourceHierarchyCarrier { .. }
                )
            })
        {
            self.source_hierarchy_carriers.insert(ty.name.clone());
            self.source_hierarchy_carriers.insert(simple.clone());
        }
        let placement = storage_facts.storage_placement_for(&ty.name);
        if placement.shared() && placement.is_implemented() {
            self.implemented_reference_identity.insert(ty.name.clone());
            self.implemented_reference_identity.insert(simple.clone());
        }
        if let Some(extends) = &ty.extends {
            self.extends
                .insert(simple.clone(), simple_type_name(extends).to_string());
        }
        for member in &ty.members {
            match member {
                Member::Method(method)
                    if crate::semantic_analysis::method_overrides_object_equals(
                        ty,
                        method,
                        program,
                        storage_facts,
                    ) =>
                {
                    self.declares_equals.insert(simple.clone());
                }
                Member::Method(method) if method_returns_concrete_comparator(method) => {
                    self.concrete_comparator_factory_methods
                        .insert((method.name.clone(), method.params.len()));
                    if let Some(rust_name) = &method.rust_name {
                        self.concrete_comparator_factory_methods
                            .insert((rust_name.clone(), method.params.len()));
                    }
                }
                Member::Type(nested) => {
                    self.collect_type(nested, program, storage_facts, representations)
                }
                _ => {}
            }
        }
    }

    /// A source class that inherits `Object.equals` rather than overriding it,
    /// anywhere up its `extends` chain.
    fn inherits_object_equals(&self, type_name: &str) -> bool {
        let mut current = simple_type_name(type_name).to_string();
        let mut seen = std::collections::BTreeSet::new();
        if !self.declared.contains(&current) {
            return false;
        }
        while seen.insert(current.clone()) {
            if self.declares_equals.contains(&current) {
                return false;
            }
            match self.extends.get(&current) {
                // An unknown supertype may well override it; saying nothing is
                // the conservative direction for a warning.
                Some(next) if !self.declared.contains(next) => return false,
                Some(next) => current = next.clone(),
                None => return true,
            }
        }
        false
    }

    fn java_type_has_implemented_reference_identity(&self, ty: &JavaType) -> bool {
        match ty {
            JavaType::Named { name, .. } => self.type_name_has_implemented_reference_identity(name),
            _ => false,
        }
    }

    fn type_name_has_implemented_reference_identity(&self, name: &str) -> bool {
        self.implemented_reference_identity.contains(name)
            || self
                .implemented_reference_identity
                .contains(simple_type_name(name))
    }

    fn source_class_has_subtype(&self, ty: &JavaType) -> bool {
        let Some(name) = concrete_java_type_name(ty) else {
            return false;
        };
        let simple = simple_type_name(name);
        self.source_classes.contains(simple) && self.extends.values().any(|parent| parent == simple)
    }

    fn source_class_reaches_supertype(&self, source_name: &str, target_name: &str) -> bool {
        let mut current = simple_type_name(source_name).to_string();
        let target = simple_type_name(target_name);
        if current == target || !self.source_classes.contains(&current) {
            return false;
        }
        let mut seen = std::collections::BTreeSet::new();
        while seen.insert(current.clone()) {
            let Some(parent) = self.extends.get(&current) else {
                return false;
            };
            if parent == target {
                return true;
            }
            current = parent.clone();
        }
        false
    }

    fn source_class_upcast_is_implemented(&self, source_name: &str, target_name: &str) -> bool {
        self.source_class_reaches_supertype(source_name, target_name)
            && (self.source_hierarchy_carriers.contains(target_name)
                || self
                    .source_hierarchy_carriers
                    .contains(simple_type_name(target_name)))
    }

    fn source_supertype_carrier_is_implemented(&self, target_name: &str) -> bool {
        self.source_hierarchy_carriers.contains(target_name)
            || self
                .source_hierarchy_carriers
                .contains(simple_type_name(target_name))
    }

    fn reference_identity_equality_is_implemented(
        &self,
        left: &JavaType,
        right: &JavaType,
    ) -> bool {
        let mut saw_reference = false;
        for ty in [left, right] {
            if java_type_has_reference_identity(ty) {
                saw_reference = true;
                if !self.java_type_has_implemented_reference_identity(ty) {
                    return false;
                }
            }
        }
        saw_reference
    }
}

fn simple_type_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn concrete_java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        _ => None,
    }
}

fn collect_type_reference_identity_equality_diagnostics(
    ty: &TypeDecl,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut field_env = TypeEnv::new();
    for (index, name) in ty.type_params.iter().enumerate() {
        field_env.insert_type_param_declaration(
            name.clone(),
            ty.type_param_bounds.get(index).cloned().flatten(),
        );
    }
    for member in &ty.members {
        if let Member::Field(field) = member {
            field_env.insert_field(field.name.clone(), field.ty.clone());
            field_env.insert_type_field(ty.name.clone(), field.name.clone(), field.ty.clone());
        }
    }
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut env = field_env.clone();
                    for (index, name) in method.type_params.iter().enumerate() {
                        env.insert_type_param_declaration(
                            name.clone(),
                            method.type_param_bounds.get(index).cloned().flatten(),
                        );
                    }
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    collect_block_reference_identity_equality_diagnostics(
                        body,
                        &mut env,
                        facts,
                        identity,
                        diagnostics,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut env = field_env.clone();
                    for param in &constructor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    collect_block_reference_identity_equality_diagnostics(
                        body,
                        &mut env,
                        facts,
                        identity,
                        diagnostics,
                    );
                }
            }
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_reference_identity_equality_diagnostics(
                        initializer,
                        &field_env,
                        facts,
                        identity,
                        diagnostics,
                    );
                }
            }
            Member::Type(nested) => collect_type_reference_identity_equality_diagnostics(
                nested,
                facts,
                identity,
                diagnostics,
            ),
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_reference_identity_equality_diagnostics(
    block: &Block,
    env: &mut TypeEnv,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    for stmt in &block.statements {
        collect_stmt_reference_identity_equality_diagnostics(
            stmt,
            env,
            facts,
            identity,
            diagnostics,
        );
    }
}

fn collect_stmt_reference_identity_equality_diagnostics(
    stmt: &Stmt,
    env: &mut TypeEnv,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
            ..
        } => {
            if let Some(initializer) = initializer {
                collect_expr_reference_identity_equality_diagnostics(
                    initializer,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            env.insert_local(name.clone(), ty.clone());
        }
        Stmt::Assign { target, value } => {
            collect_expr_reference_identity_equality_diagnostics(
                target,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                value,
                env,
                facts,
                identity,
                diagnostics,
            );
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_reference_identity_equality_diagnostics(
                expr,
                env,
                facts,
                identity,
                diagnostics,
            );
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_used_pattern_instanceof_supertype_diagnostic(
                condition,
                Some(then_block),
                None,
                env,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                condition,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_block_reference_identity_equality_diagnostics(
                then_block,
                &mut env.clone(),
                facts,
                identity,
                diagnostics,
            );
            if let Some(else_block) = else_block {
                collect_block_reference_identity_equality_diagnostics(
                    else_block,
                    &mut env.clone(),
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_reference_identity_equality_diagnostics(
                condition,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_block_reference_identity_equality_diagnostics(
                body,
                &mut env.clone(),
                facts,
                identity,
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
            let mut loop_env = env.clone();
            for init_stmt in init {
                collect_stmt_reference_identity_equality_diagnostics(
                    init_stmt,
                    &mut loop_env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                collect_expr_reference_identity_equality_diagnostics(
                    condition,
                    &loop_env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            for update_expr in update {
                collect_expr_reference_identity_equality_diagnostics(
                    update_expr,
                    &loop_env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            collect_block_reference_identity_equality_diagnostics(
                body,
                &mut loop_env,
                facts,
                identity,
                diagnostics,
            );
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            collect_expr_reference_identity_equality_diagnostics(
                iterable,
                env,
                facts,
                identity,
                diagnostics,
            );
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            collect_block_reference_identity_equality_diagnostics(
                body,
                &mut body_env,
                facts,
                identity,
                diagnostics,
            );
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_reference_identity_equality_diagnostics(
                selector,
                env,
                facts,
                identity,
                diagnostics,
            );
            for case in cases {
                collect_switch_case_reference_identity_equality_diagnostics(
                    case,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_env = env.clone();
            for resource in resources {
                collect_stmt_reference_identity_equality_diagnostics(
                    resource,
                    &mut try_env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            collect_block_reference_identity_equality_diagnostics(
                body,
                &mut try_env,
                facts,
                identity,
                diagnostics,
            );
            for catch in catches {
                collect_catch_reference_identity_equality_diagnostics(
                    catch,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_block_reference_identity_equality_diagnostics(
                    finally_block,
                    &mut env.clone(),
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_type_reference_identity_equality_diagnostics(ty, facts, identity, diagnostics);
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(
                    arg,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_reference_identity_equality_diagnostics(
                block,
                &mut env.clone(),
                facts,
                identity,
                diagnostics,
            );
        }
    }
}

fn collect_switch_case_reference_identity_equality_diagnostics(
    case: &SwitchCase,
    env: &TypeEnv,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut case_env = env.clone();
    for label in &case.labels {
        if let crate::java_ir::SwitchLabel::Expr(expr) = label {
            collect_expr_reference_identity_equality_diagnostics(
                expr,
                &case_env,
                facts,
                identity,
                diagnostics,
            );
        }
    }
    for stmt in &case.statements {
        collect_stmt_reference_identity_equality_diagnostics(
            stmt,
            &mut case_env,
            facts,
            identity,
            diagnostics,
        );
    }
}

fn collect_catch_reference_identity_equality_diagnostics(
    catch: &CatchClause,
    env: &TypeEnv,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut catch_env = env.clone();
    let catch_ty = catch
        .types
        .first()
        .cloned()
        .unwrap_or_else(|| JavaType::Unknown("Throwable".to_string()));
    catch_env.insert_local(catch.source_name.clone(), catch_ty);
    collect_block_reference_identity_equality_diagnostics(
        &catch.body,
        &mut catch_env,
        facts,
        identity,
        diagnostics,
    );
}

fn priority_queue_comparator_arg_is_supported_source_factory(
    args: &[Expr],
    identity: &SourceIdentityFacts,
) -> bool {
    let [
        Expr::Call {
            name,
            args: call_args,
            ..
        },
    ] = args
    else {
        return false;
    };
    identity
        .concrete_comparator_factory_methods
        .contains(&(name.clone(), call_args.len()))
}

fn method_returns_concrete_comparator(method: &MethodDecl) -> bool {
    java_type_is_comparator(&method.return_type)
        && method.body.as_ref().is_some_and(|body| {
            let mut returned_types = Vec::new();
            collect_concrete_comparator_return_types_from_block(body, &mut returned_types)
                && !returned_types.is_empty()
                && returned_types.iter().all(|ty| ty == &returned_types[0])
        })
}

fn collect_concrete_comparator_return_types_from_block(
    block: &Block,
    out: &mut Vec<JavaType>,
) -> bool {
    block
        .statements
        .iter()
        .all(|stmt| collect_concrete_comparator_return_types_from_stmt(stmt, out))
}

fn collect_concrete_comparator_return_types_from_stmt(
    stmt: &Stmt,
    out: &mut Vec<JavaType>,
) -> bool {
    match stmt {
        Stmt::Return(Some(expr)) => {
            let Some(ty) = concrete_comparator_initializer_type(expr) else {
                return false;
            };
            out.push(ty);
            true
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_concrete_comparator_return_types_from_block(then_block, out)
                && else_block.as_ref().is_none_or(|block| {
                    collect_concrete_comparator_return_types_from_block(block, out)
                })
        }
        Stmt::Switch { cases, .. } => cases.iter().all(|case| {
            case.statements
                .iter()
                .all(|stmt| collect_concrete_comparator_return_types_from_stmt(stmt, out))
        }),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            collect_concrete_comparator_return_types_from_block(body, out)
                && catches.iter().all(|catch| {
                    collect_concrete_comparator_return_types_from_block(&catch.body, out)
                })
                && finally_block.as_ref().is_none_or(|block| {
                    collect_concrete_comparator_return_types_from_block(block, out)
                })
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_concrete_comparator_return_types_from_block(block, out)
        }
        Stmt::Return(None) => false,
        _ => true,
    }
}

fn concrete_comparator_initializer_type(initializer: &Expr) -> Option<JavaType> {
    match initializer {
        Expr::New {
            ty,
            body: None,
            resolved,
            ..
        } if !java_type_is_comparator(ty) => {
            if matches!(ty, JavaType::Unknown(_))
                && let Some(resolved) = resolved
            {
                Some(JavaType::Named {
                    name: resolved.declaring_type.clone(),
                    args: Vec::new(),
                })
            } else {
                Some(ty.clone())
            }
        }
        Expr::Cast { ty, expr } if java_type_is_comparator(ty) => {
            concrete_comparator_initializer_type(expr)
        }
        _ => None,
    }
}

fn java_type_is_comparator(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
            canonical
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Comparator")
        }
        _ => false,
    }
}

fn collect_expr_reference_identity_equality_diagnostics(
    expr: &Expr,
    env: &TypeEnv,
    facts: &FactSet,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    if let Expr::New { ty, args, .. } = expr
        && matches!(
            plan_native_collection_constructor_for_args(ty, args, env, facts),
            Some(NativeCollectionNewTarget::PriorityQueueComparatorUnsupported { .. })
        )
        && !priority_queue_comparator_arg_is_supported_source_factory(args, identity)
    {
        diagnostics.push(LoweringDiagnosticPlan::new(
            Severity::Unsupported,
            "unsupported_priority_queue_comparator_constructor",
            "Java PriorityQueue(Comparator) requires comparator-backed priority-queue support"
                .to_string(),
        ));
    }
    if let Expr::Binary { op, left, right } = expr
        && matches!(op, BinaryOp::Eq | BinaryOp::Ne)
        && !expr_is_null_literal(left)
        && !expr_is_null_literal(right)
    {
        let left_ty = infer_expr_type(left, env);
        let right_ty = infer_expr_type(right, env);
        let generic_identity = left_ty == right_ty && {
            let ty =
                crate::types::optional_element_type(&left_ty).unwrap_or_else(|| left_ty.clone());
            match ty {
                JavaType::Named { name, args } if args.is_empty() => {
                    env.is_type_parameter_name(&name)
                }
                JavaType::Unknown(name) => env.is_type_parameter_name(&name),
                _ => false,
            }
        };
        if !generic_identity
            && let Some(code) =
                reference_identity_equality_diagnostic_code(&left_ty, &right_ty, identity)
        {
            diagnostics.push(LoweringDiagnosticPlan::new(
                Severity::Warning,
                code,
                "Java ==/!= on reference type is currently lowered as Rust equality; object identity is not represented yet".to_string(),
            ));
        }
    }
    if let Expr::Cast { ty, expr: source } = expr
        && erased_object_cast_to_supertype_requires_hierarchy_diagnostic(source, ty, env, identity)
    {
        diagnostics.push(LoweringDiagnosticPlan::new(
            Severity::Unsupported,
            "unsupported_cast_source_hierarchy_runtime_carrier",
            format!(
                "unsupported Java cast: target type {} has source subtypes and requires class hierarchy lowering",
                concrete_java_type_name(ty).unwrap_or("unknown")
            ),
        ));
    } else if let Expr::Cast { ty, expr: source } = expr
        && source_class_cast_to_supertype_requires_hierarchy_diagnostic(source, ty, env, identity)
    {
        diagnostics.push(LoweringDiagnosticPlan::new(
            Severity::Unsupported,
            "unsupported_inheritance",
            format!(
                "unsupported Java class upcast: {} to {} requires class hierarchy lowering",
                concrete_java_type_name(&infer_expr_type(source, env)).unwrap_or("unknown"),
                concrete_java_type_name(ty).unwrap_or("unknown")
            ),
        ));
    }
    // `a.equals(b)` on a class that does not override `equals` is *identity* in
    // Java, not structural comparison. Same defect as `==`, and worse in effect:
    // it answers `true` where Java says `false`, so a `HashMap` or `contains`
    // built on it merges entries Java keeps apart.
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && name == "equals"
        && args.len() == 1
        && !expr_is_null_literal(&args[0])
        && let JavaType::Named {
            name: receiver_type,
            ..
        } = infer_expr_type(receiver, env)
        && identity.inherits_object_equals(&receiver_type)
        && !identity.type_name_has_implemented_reference_identity(&receiver_type)
    {
        diagnostics.push(LoweringDiagnosticPlan::new(
            Severity::Warning,
            "java_inherited_equals_identity",
            format!(
                "{receiver_type} does not override equals, so Java compares references here; \
                 lowering compares fields, which reports equal for distinct objects"
            ),
        ));
    }
    match expr {
        Expr::FieldAccess(receiver, _)
        | Expr::MethodReference {
            receiver: Some(receiver),
            ..
        } => collect_expr_reference_identity_equality_diagnostics(
            receiver,
            env,
            facts,
            identity,
            diagnostics,
        ),
        Expr::ArrayAccess(array, index) => {
            collect_expr_reference_identity_equality_diagnostics(
                array,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                index,
                env,
                facts,
                identity,
                diagnostics,
            );
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_reference_identity_equality_diagnostics(
                    dimension,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    collect_expr_reference_identity_equality_diagnostics(
                        item,
                        env,
                        facts,
                        identity,
                        diagnostics,
                    );
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_expr_reference_identity_equality_diagnostics(
                    item,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_reference_identity_equality_diagnostics(
                left,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                right,
                env,
                facts,
                identity,
                diagnostics,
            );
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_reference_identity_equality_diagnostics(
                expr,
                env,
                facts,
                identity,
                diagnostics,
            )
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_used_pattern_instanceof_supertype_diagnostic(
                condition,
                None,
                Some(then_expr),
                env,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                condition,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                then_expr,
                env,
                facts,
                identity,
                diagnostics,
            );
            collect_expr_reference_identity_equality_diagnostics(
                else_expr,
                env,
                facts,
                identity,
                diagnostics,
            );
        }
        Expr::Switch { selector, cases } => {
            collect_expr_reference_identity_equality_diagnostics(
                selector,
                env,
                facts,
                identity,
                diagnostics,
            );
            for case in cases {
                collect_expr_reference_identity_equality_diagnostics(
                    &case.value,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_reference_identity_equality_diagnostics(
                    receiver,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(
                    arg,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(
                    arg,
                    env,
                    facts,
                    identity,
                    diagnostics,
                );
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Type(ty) = member {
                        collect_type_reference_identity_equality_diagnostics(
                            ty,
                            facts,
                            identity,
                            diagnostics,
                        );
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_expr_reference_identity_equality_diagnostics(
                expr,
                env,
                facts,
                identity,
                diagnostics,
            ),
            LambdaBody::Block(block) => collect_block_reference_identity_equality_diagnostics(
                block,
                &mut env.clone(),
                facts,
                identity,
                diagnostics,
            ),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

fn collect_used_pattern_instanceof_supertype_diagnostic(
    condition: &Expr,
    then_block: Option<&Block>,
    then_expr: Option<&Expr>,
    env: &TypeEnv,
    identity: &SourceIdentityFacts,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let Some((source, ty, binding, guard)) = pattern_instanceof_condition_parts(condition) else {
        return;
    };
    if !guard.is_some_and(|guard| expr_reads_name(guard, binding))
        && !then_block.is_some_and(|block| block_reads_name(block, binding))
        && !then_expr.is_some_and(|expr| expr_reads_name(expr, binding))
    {
        return;
    }
    if source_instanceof_to_supertype_requires_hierarchy_diagnostic(source, ty, env, identity) {
        diagnostics.push(LoweringDiagnosticPlan::new(
            Severity::Unsupported,
            "unsupported_instanceof_source_hierarchy_runtime_carrier",
            format!(
                "unsupported Java instanceof check: target type {} has source subtypes and requires class hierarchy lowering",
                concrete_java_type_name(ty).unwrap_or("unknown")
            ),
        ));
    }
}

fn pattern_instanceof_condition_parts(
    condition: &Expr,
) -> Option<(&Expr, &JavaType, &str, Option<&Expr>)> {
    match condition {
        Expr::InstanceOf {
            expr,
            ty,
            binding: Some(binding),
        } => Some((expr, ty, binding, None)),
        Expr::Binary {
            op: BinaryOp::And,
            left,
            right,
        } => {
            let Expr::InstanceOf {
                expr,
                ty,
                binding: Some(binding),
            } = left.as_ref()
            else {
                return None;
            };
            Some((expr, ty, binding, Some(right)))
        }
        _ => None,
    }
}

fn block_reads_name(block: &Block, name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_reads_name(stmt, name))
}

fn stmt_reads_name(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_reads_name(expr, name)),
        Stmt::Assign { target, value } => {
            expr_reads_name(target, name) || expr_reads_name(value, name)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_reads_name(expr, name)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_reads_name(condition, name)
                || block_reads_name(then_block, name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_reads_name(block, name))
        }
        Stmt::While {
            condition, body, ..
        } => expr_reads_name(condition, name) || block_reads_name(body, name),
        Stmt::DoWhile {
            body, condition, ..
        } => block_reads_name(body, name) || expr_reads_name(condition, name),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| stmt_reads_name(stmt, name))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_reads_name(expr, name))
                || update.iter().any(|expr| expr_reads_name(expr, name))
                || block_reads_name(body, name)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_reads_name(iterable, name) || block_reads_name(body, name)
        }
        Stmt::Switch { selector, cases } => {
            expr_reads_name(selector, name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => expr_reads_name(expr, name),
                        SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_name(stmt, name))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|resource| stmt_reads_name(resource, name))
                || block_reads_name(body, name)
                || catches
                    .iter()
                    .any(|catch| block_reads_name(&catch.body, name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_reads_name(block, name))
        }
        Stmt::LocalType(ty) => ty
            .members
            .iter()
            .any(|member| member_reads_name(member, name)),
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(|expr| expr_reads_name(expr, name))
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_reads_name(block, name)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_reads_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(candidate) => candidate == name,
        Expr::FieldAccess(receiver, _)
        | Expr::MethodReference {
            receiver: Some(receiver),
            ..
        } => expr_reads_name(receiver, name),
        Expr::ArrayAccess(array, index) => {
            expr_reads_name(array, name) || expr_reads_name(index, name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(|expr| expr_reads_name(expr, name))
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(|expr| expr_reads_name(expr, name)))
        }
        Expr::ArrayInitializer(items) => items.iter().any(|expr| expr_reads_name(expr, name)),
        Expr::Binary { left, right, .. } => {
            expr_reads_name(left, name) || expr_reads_name(right, name)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_reads_name(expr, name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_reads_name(condition, name)
                || expr_reads_name(then_expr, name)
                || expr_reads_name(else_expr, name)
        }
        Expr::Switch { selector, cases } => {
            expr_reads_name(selector, name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => expr_reads_name(expr, name),
                        SwitchLabel::Default => false,
                    }) || expr_reads_name(&case.value, name)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_reads_name(receiver, name))
                || args.iter().any(|expr| expr_reads_name(expr, name))
        }
        Expr::New { args, body, .. } => {
            args.iter().any(|expr| expr_reads_name(expr, name))
                || body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(|member| member_reads_name(member, name)))
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => expr_reads_name(expr, name),
            LambdaBody::Block(block) => block_reads_name(block, name),
        },
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

fn member_reads_name(member: &Member, name: &str) -> bool {
    match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_reads_name(expr, name)),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|block| block_reads_name(block, name)),
        Member::Constructor(ctor) => ctor
            .body
            .as_ref()
            .is_some_and(|block| block_reads_name(block, name)),
        Member::Type(ty) => ty
            .members
            .iter()
            .any(|member| member_reads_name(member, name)),
        Member::Unsupported { .. } => false,
    }
}

fn source_instanceof_to_supertype_requires_hierarchy_diagnostic(
    source: &Expr,
    target: &JavaType,
    env: &TypeEnv,
    identity: &SourceIdentityFacts,
) -> bool {
    matches!(
        infer_expr_type(source, env),
        JavaType::Named { name, .. } if simple_type_name(&name) == "Object"
    ) && identity.source_class_has_subtype(target)
        && concrete_java_type_name(target).is_none_or(|target_name| {
            !identity.source_supertype_carrier_is_implemented(target_name)
        })
}

fn erased_object_cast_to_supertype_requires_hierarchy_diagnostic(
    source: &Expr,
    target: &JavaType,
    env: &TypeEnv,
    identity: &SourceIdentityFacts,
) -> bool {
    let _ = (source, target, env, identity);
    false
}

fn source_class_cast_to_supertype_requires_hierarchy_diagnostic(
    source: &Expr,
    target: &JavaType,
    env: &TypeEnv,
    identity: &SourceIdentityFacts,
) -> bool {
    let source_ty = infer_expr_type(source, env);
    let Some(source_name) = concrete_java_type_name(&source_ty) else {
        return false;
    };
    let Some(target_name) = concrete_java_type_name(target) else {
        return false;
    };
    identity.source_class_reaches_supertype(source_name, target_name)
        && !identity.source_class_upcast_is_implemented(source_name, target_name)
}

fn reference_identity_equality_diagnostic_code(
    left: &JavaType,
    right: &JavaType,
    identity: &SourceIdentityFacts,
) -> Option<&'static str> {
    if left == &JavaType::String || right == &JavaType::String {
        return Some("java_string_identity");
    }
    if java_type_has_reference_identity(left) || java_type_has_reference_identity(right) {
        if identity.reference_identity_equality_is_implemented(left, right) {
            return None;
        }
        return Some("java_reference_identity_equality");
    }
    None
}

fn java_type_has_reference_identity(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Named { .. } | JavaType::Array(_))
}

fn expr_is_null_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(crate::java_ir::Literal::Null))
}

#[cfg(test)]
mod tests {
    use crate::translate_source;

    #[test]
    fn identity_diagnostics_do_not_treat_source_object_overloads_as_overrides() {
        for (source, expected_identity) in [
            (
                "class Object {} class C { public boolean equals(Object x) { return true; } }",
                true,
            ),
            (
                "class C { public boolean equals(other.Object x) { return true; } }",
                true,
            ),
            (
                "import other.Object; class C { public boolean equals(Object x) { return true; } }",
                true,
            ),
            (
                "class Object {} class C { public boolean equals(java.lang.Object x) { return true; } }",
                false,
            ),
            (
                "class C { public boolean equals(Object x) { return true; } }",
                false,
            ),
        ] {
            let parsed = crate::parser::parse_java_source(source);
            let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
            let semantic = crate::semantic_analysis::analyze_program(
                &resolved.program,
                &resolved.facts,
                &crate::facts::FactSet::default(),
            );
            let representations =
                crate::representation::plan_representations(&resolved.program, &semantic.facts);
            let identity = super::SourceIdentityFacts::from_program(
                &resolved.program,
                &semantic.facts,
                &representations,
            );
            assert_eq!(
                identity.inherits_object_equals("C"),
                expected_identity,
                "{source}"
            );
        }
    }

    #[test]
    fn explicit_source_supertype_cast_with_carrier_reports_no_inheritance_gap() {
        let translation = translate_source(
            r#"
class ExplicitInheritanceUpcast {
    static String run() {
        Account a = (Account) new Savings(100, 5);
        return a.label();
    }
}
class Account {
    int amount;
    Account(int amount) { this.amount = amount; }
    String label() { return "account"; }
}
class Savings extends Account {
    int rate;
    Savings(int amount, int rate) { super(amount); this.rate = rate; }
    String label() { return "savings"; }
}
"#,
        );

        assert!(
            !translation.diagnostics.has_code("unsupported_inheritance"),
            "{:?}",
            translation.diagnostics.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn erased_object_supertype_cast_with_carrier_reports_no_runtime_gap() {
        let translation = translate_source(
            r#"
class ErasedObjectSupertypeCast {
    static String run() {
        Object o = new B();
        A a = (A) o;
        return a.label();
    }
}
class A {
    String label() { return "A"; }
}
class B extends A {
    String label() { return "B"; }
}
"#,
        );

        assert!(
            !translation
                .diagnostics
                .has_code("unsupported_cast_source_hierarchy_runtime_carrier"),
            "{:?}",
            translation.diagnostics.iter().collect::<Vec<_>>()
        );
    }
}
