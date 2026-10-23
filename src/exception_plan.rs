use std::collections::{BTreeMap, HashMap, HashSet};

use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, Block, CatchClause, Expr, JavaType, Member, MethodDecl, Modifier, Program, Stmt,
    SwitchCase, SwitchLabel, TypeDecl, TypeKind,
};
use crate::java_signature::{JavaCallableSlotKey, source_method_rust_name};
use crate::types::TypeEnv;
use crate::usage::{CallGraph, CallableKey};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ThrowExprPlan<'a> {
    NewException {
        class_name: String,
        args: &'a [Expr],
    },
    CauseOrSelfIfInstance {
        wrapper: &'a Expr,
        class_names: Vec<String>,
    },
    ExistingException(&'a Expr),
    FallbackValue(&'a Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaExceptionSupportPlan {
    emit_support_item: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodExceptionPlan {
    wrap_returns_in_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramExceptionPlan {
    support: JavaExceptionSupportPlan,
    methods: BTreeMap<(String, String), MethodExceptionPlan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramFallibilityPlan {
    callables: HashMap<CallableKey, CallableFallibility>,
    solve_iterations: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallableFallibility {
    returns_result: bool,
    reasons: Vec<FallibilityReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallibilityReason {
    /// Compiler-provided enum lookup rejects unknown names and null inputs.
    EnumLookupThrow,
    /// Integer `/` or `%` can throw ArithmeticException on a zero divisor.
    IntegerDivRemThrow,
    /// Math integer helpers can throw ArithmeticException.
    CheckedNativeArithmeticThrow,
    /// A nullable Java reference was dereferenced as a receiver and can throw
    /// NullPointerException.
    NullReceiverThrow,
    /// A checked identity rejects a nullable argument with NullPointerException.
    NullCheckThrow,
    /// A checked reference conversion can throw ClassCastException.
    ClassCastThrow,
    /// Array reads and writes can fail bounds checks even for non-null arrays.
    ArrayBoundsThrow,
    /// Native array-copy factories have checked size/range/null failures.
    NativeArrayCopyThrow,
    /// Shared native collections can reject mutation or checked list indices.
    RetainedNativeCollectionThrow,
    /// A retained functional value has an open runtime callback body.
    StoredCallbackThrow,
    /// Public/protected invocation-only callback parameters admit callers
    /// outside this translation unit, whose bodies are not proven infallible.
    OpenCallbackParameterThrow,
    /// Implicit Java iteration invokes an open, checked iterator protocol.
    IteratorProtocolThrow,
    /// Open Java Object dispatch (toString/hashCode) can execute an overriding body.
    VirtualObjectDispatchThrow,
    /// Semantic set membership invokes the query's potentially throwing equals.
    SemanticSetKeyThrow,
    /// A Java `throw` statement escapes the callable. Runtime exceptions are
    /// unchecked by javac, but they are still catchable Java control flow.
    DirectThrow,
    /// Equalized across an override set: a trait method and its implementations
    /// must share one Rust signature.
    PropagatedFromOverride,
    MethodThrowFact,
    ExternalThrowFact,
    PropagatedFromCallee,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TryStmtPlan<'a> {
    CatchOnly {
        body: &'a Block,
        catches: &'a [CatchClause],
        completion: TryBlockCompletion,
    },
    WithResources {
        resources: &'a [Stmt],
        body: &'a Block,
        catches: &'a [CatchClause],
        completion: TryBlockCompletion,
    },
    WithResourcesFinally {
        resources: &'a [Stmt],
        body: &'a Block,
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    WithResourcesCatchFinally {
        resources: &'a [Stmt],
        body: &'a Block,
        catches: &'a [CatchClause],
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    FinallyOnly {
        body: &'a Block,
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    CatchFinally {
        body: &'a Block,
        catches: &'a [CatchClause],
        finally_block: &'a Block,
        completion: TryBlockCompletion,
    },
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryBlockCompletion {
    Terminates,
    FallsThroughUnit,
}

impl ProgramExceptionPlan {
    pub fn support(&self) -> JavaExceptionSupportPlan {
        self.support
    }

    pub fn emits_support_item(&self) -> bool {
        self.support.emit_support_item()
    }

    pub fn method_plan(&self, ty: &TypeDecl, method: &MethodDecl) -> MethodExceptionPlan {
        self.methods
            .get(&method_exception_key(ty, method))
            .copied()
            .unwrap_or_else(|| MethodExceptionPlan::new(false))
    }
}

impl JavaExceptionSupportPlan {
    pub fn new(emit_support_item: bool) -> Self {
        Self { emit_support_item }
    }

    pub fn emit_support_item(&self) -> bool {
        self.emit_support_item
    }
}

impl MethodExceptionPlan {
    pub fn new(wrap_returns_in_result: bool) -> Self {
        Self {
            wrap_returns_in_result,
        }
    }

    pub fn wrap_returns_in_result(&self) -> bool {
        self.wrap_returns_in_result
    }
}

impl ProgramFallibilityPlan {
    pub fn callable(&self, key: &CallableKey) -> Option<&CallableFallibility> {
        self.callables.get(key)
    }

    pub fn returns_result(&self, key: &CallableKey) -> bool {
        self.callable(key)
            .is_some_and(CallableFallibility::returns_result)
    }

    pub fn solve_iterations(&self) -> usize {
        self.solve_iterations
    }

    /// Marks a callable fallible. Test-only: the solver is the sole writer in
    /// real use, which is the property 0010 is about.
    #[cfg(test)]
    pub(crate) fn mark_returns_result_for_test(&mut self, key: CallableKey) {
        self.callables.entry(key).or_default().returns_result = true;
    }
}

impl CallableFallibility {
    pub fn returns_result(&self) -> bool {
        self.returns_result
    }

    pub fn reasons(&self) -> &[FallibilityReason] {
        &self.reasons
    }
}

impl Default for ProgramExceptionPlan {
    fn default() -> Self {
        Self {
            support: JavaExceptionSupportPlan::new(false),
            methods: BTreeMap::new(),
        }
    }
}

pub(crate) fn program_uses_java_exception_with_facts(program: &Program, facts: &FactSet) -> bool {
    facts.has_method_throw_effects()
        || !facts.exception_facts.is_empty()
        || !facts.catch_facts.is_empty()
        || program_has_lowerable_try_scope(program)
        || program_has_integer_div_rem_expr(program)
        || program_has_checked_native_arithmetic_call(program)
}

pub fn plan_java_exception_support(program: &Program, facts: &FactSet) -> JavaExceptionSupportPlan {
    JavaExceptionSupportPlan::new(program_uses_java_exception_with_facts(program, facts))
}

fn program_has_lowerable_try_scope(program: &Program) -> bool {
    program.types.iter().any(type_has_lowerable_try_scope)
}

fn type_has_lowerable_try_scope(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_lowerable_try_scope),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_lowerable_try_scope),
        Member::Type(nested) => type_has_lowerable_try_scope(nested),
        Member::Field(_) | Member::Unsupported { .. } => false,
    })
}

fn block_has_lowerable_try_scope(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_lowerable_try_scope)
}

fn stmt_has_lowerable_try_scope(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            !matches!(
                plan_try_stmt(resources, body, catches, finally_block.as_ref()),
                TryStmtPlan::Unsupported
            ) || block_has_lowerable_try_scope(body)
                || catches
                    .iter()
                    .any(|catch| block_has_lowerable_try_scope(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_lowerable_try_scope)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_lowerable_try_scope(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_lowerable_try_scope)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_has_lowerable_try_scope(body),
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_has_lowerable_try_scope)),
        Stmt::LocalType(nested) => type_has_lowerable_try_scope(nested),
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn program_has_integer_div_rem_expr(program: &Program) -> bool {
    program.types.iter().any(type_has_integer_div_rem_expr)
}

fn type_has_integer_div_rem_expr(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_integer_div_rem_expr),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_integer_div_rem_expr),
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_has_integer_div_rem_expr),
        Member::Type(nested) => type_has_integer_div_rem_expr(nested),
        Member::Unsupported { .. } => false,
    })
}

fn block_has_integer_div_rem_expr(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_integer_div_rem_expr)
}

fn stmt_has_integer_div_rem_expr(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => initializer
            .as_ref()
            .is_some_and(expr_has_integer_div_rem_expr),
        Stmt::Assign { target, value } => {
            expr_has_integer_div_rem_expr(target) || expr_has_integer_div_rem_expr(value)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_has_integer_div_rem_expr(expr),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_integer_div_rem_expr(condition)
                || block_has_integer_div_rem_expr(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_integer_div_rem_expr)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_has_integer_div_rem_expr(condition) || block_has_integer_div_rem_expr(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_has_integer_div_rem_expr)
                || condition
                    .as_ref()
                    .is_some_and(expr_has_integer_div_rem_expr)
                || update.iter().any(expr_has_integer_div_rem_expr)
                || block_has_integer_div_rem_expr(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_integer_div_rem_expr(iterable) || block_has_integer_div_rem_expr(body)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_has_integer_div_rem_expr(block)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_integer_div_rem_expr(selector)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(switch_label_has_integer_div_rem_expr)
                        || case.statements.iter().any(stmt_has_integer_div_rem_expr)
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_has_integer_div_rem_expr)
                || block_has_integer_div_rem_expr(body)
                || catches
                    .iter()
                    .any(|catch| block_has_integer_div_rem_expr(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_integer_div_rem_expr)
        }
        Stmt::LocalType(nested) => type_has_integer_div_rem_expr(nested),
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_has_integer_div_rem_expr),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_has_integer_div_rem_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Binary { op, left, right } => {
            matches!(op, BinaryOp::Div | BinaryOp::Rem)
                || expr_has_integer_div_rem_expr(left)
                || expr_has_integer_div_rem_expr(right)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::FieldAccess(expr, _) => {
            expr_has_integer_div_rem_expr(expr)
        }
        Expr::ArrayAccess(array, index) => {
            expr_has_integer_div_rem_expr(array) || expr_has_integer_div_rem_expr(index)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_has_integer_div_rem_expr(receiver))
                || args.iter().any(expr_has_integer_div_rem_expr)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_has_integer_div_rem_expr(qualifier))
                || args.iter().any(expr_has_integer_div_rem_expr)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(block_has_integer_div_rem_expr),
                        Member::Constructor(constructor) => constructor
                            .body
                            .as_ref()
                            .is_some_and(block_has_integer_div_rem_expr),
                        Member::Field(field) => field
                            .initializer
                            .as_ref()
                            .is_some_and(expr_has_integer_div_rem_expr),
                        Member::Type(nested) => type_has_integer_div_rem_expr(nested),
                        Member::Unsupported { .. } => false,
                    })
                })
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_has_integer_div_rem_expr)
                || initializer
                    .as_ref()
                    .is_some_and(|elements| elements.iter().any(expr_has_integer_div_rem_expr))
        }
        Expr::ArrayInitializer(elements) => elements.iter().any(expr_has_integer_div_rem_expr),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_integer_div_rem_expr(condition)
                || expr_has_integer_div_rem_expr(then_expr)
                || expr_has_integer_div_rem_expr(else_expr)
        }
        Expr::Switch { selector, cases } => {
            expr_has_integer_div_rem_expr(selector)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(switch_label_has_integer_div_rem_expr)
                        || expr_has_integer_div_rem_expr(&case.value)
                })
        }
        Expr::InstanceOf { expr, .. } => expr_has_integer_div_rem_expr(expr),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_has_integer_div_rem_expr(expr),
            crate::java_ir::LambdaBody::Block(block) => block_has_integer_div_rem_expr(block),
        },
        Expr::MethodReference { .. } => false,
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn expr_has_throwing_integer_div_rem_expr(expr: &Expr, env: &TypeEnv) -> bool {
    match expr {
        Expr::Binary { op, left, right } => {
            let operation_can_throw = matches!(op, BinaryOp::Div | BinaryOp::Rem)
                && !matches!(
                    crate::types::infer_expr_type(expr, env),
                    JavaType::Primitive(
                        crate::java_ir::PrimitiveType::Float
                            | crate::java_ir::PrimitiveType::Double
                    )
                );
            operation_can_throw
                || expr_has_throwing_integer_div_rem_expr(left, env)
                || expr_has_throwing_integer_div_rem_expr(right, env)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::FieldAccess(expr, _) => {
            expr_has_throwing_integer_div_rem_expr(expr, env)
        }
        Expr::ArrayAccess(array, index) => {
            expr_has_throwing_integer_div_rem_expr(array, env)
                || expr_has_throwing_integer_div_rem_expr(index, env)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_has_throwing_integer_div_rem_expr(receiver, env))
                || args
                    .iter()
                    .any(|arg| expr_has_throwing_integer_div_rem_expr(arg, env))
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_has_throwing_integer_div_rem_expr(qualifier, env))
                || args
                    .iter()
                    .any(|arg| expr_has_throwing_integer_div_rem_expr(arg, env))
                // Initialization executes at creation; method bodies belong
                // to their synthetic callable identities and execute later.
                || body.as_ref().is_some_and(|members| {
                    let mut initializer_env = env.clone();
                    initializer_env.enter_anonymous_field_scope(members.iter().filter_map(|member| match member {
                        Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
                        _ => None,
                    }));
                    members.iter().any(|member| match member {
                        Member::Method(method) if crate::class_initialization::is_initializer(method) => method
                            .body
                            .as_ref()
                            .is_some_and(|body| block_has_unhandled_integer_div_rem_with_env(body, &initializer_env)),
                        Member::Constructor(constructor) => constructor
                            .body
                            .as_ref()
                            .is_some_and(|body| block_has_unhandled_integer_div_rem_with_env(body, &initializer_env)),
                        Member::Field(field) => field
                            .initializer
                            .as_ref()
                            .is_some_and(|expr| expr_has_throwing_integer_div_rem_expr(expr, &initializer_env)),
                        _ => false,
                    })
                })
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|dimension| expr_has_throwing_integer_div_rem_expr(dimension, env))
                || initializer.as_ref().is_some_and(|elements| {
                    elements
                        .iter()
                        .any(|element| expr_has_throwing_integer_div_rem_expr(element, env))
                })
        }
        Expr::ArrayInitializer(elements) => elements
            .iter()
            .any(|element| expr_has_throwing_integer_div_rem_expr(element, env)),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_throwing_integer_div_rem_expr(condition, env)
                || expr_has_throwing_integer_div_rem_expr(then_expr, env)
                || expr_has_throwing_integer_div_rem_expr(else_expr, env)
        }
        Expr::Switch { selector, cases } => {
            expr_has_throwing_integer_div_rem_expr(selector, env)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => {
                            expr_has_throwing_integer_div_rem_expr(expr, env)
                        }
                        SwitchLabel::Default => false,
                    }) || expr_has_throwing_integer_div_rem_expr(&case.value, env)
                })
        }
        Expr::InstanceOf { expr, .. } => expr_has_throwing_integer_div_rem_expr(expr, env),
        Expr::Lambda { .. } => false,
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn switch_label_has_integer_div_rem_expr(label: &crate::java_ir::SwitchLabel) -> bool {
    match label {
        crate::java_ir::SwitchLabel::Expr(expr) => expr_has_integer_div_rem_expr(expr),
        crate::java_ir::SwitchLabel::Default => false,
    }
}

fn program_has_checked_native_arithmetic_call(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(type_has_checked_native_arithmetic_call)
}

fn type_has_checked_native_arithmetic_call(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_checked_native_arithmetic_call),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_checked_native_arithmetic_call),
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_has_checked_native_arithmetic_call),
        Member::Type(nested) => type_has_checked_native_arithmetic_call(nested),
        Member::Unsupported { .. } => false,
    })
}

fn block_has_checked_native_arithmetic_call(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_has_checked_native_arithmetic_call)
}

/// The **only** producer of `MethodExceptionPlan`.
///
/// `plan_method_exceptions` used to derive `wrap_returns_in_result` a second
/// time from `facts.method_has_thrown_exceptions`, giving fallibility two
/// stores that could disagree — the defect
/// [0010](../claude_audits/0010-fallibility-is-decided-three-times-and-disagrees.md)
/// is named for. It had no non-test caller left, so it is deleted and the
/// solved plan is not optional.
pub fn plan_program_exceptions_with_fallibility(
    program: &Program,
    facts: &FactSet,
    fallibility: &ProgramFallibilityPlan,
) -> ProgramExceptionPlan {
    let mut methods = BTreeMap::new();
    for ty in &program.types {
        collect_method_exception_plans_for_type(ty, facts, fallibility, &mut methods);
    }
    ProgramExceptionPlan {
        support: JavaExceptionSupportPlan::new(
            program_uses_java_exception_with_facts(program, facts)
                || fallibility
                    .callables
                    .values()
                    .any(CallableFallibility::returns_result),
        ),
        methods,
    }
}

pub fn plan_program_fallibility(
    program: &Program,
    call_graph: &CallGraph,
    facts: &FactSet,
) -> ProgramFallibilityPlan {
    plan_program_fallibility_with_signatures(program, call_graph, facts, None)
}

/// Solve fallibility against the same parameter carriers that lowering emits.
///
/// Java reference parameters can receive `null`, but the signature solver only
/// uses an `Option` carrier when the translated program actually preserves that
/// part of the value domain.  Treating every declared reference as nullable
/// here made a method return `Result<T, JavaException>` even when its Rust ABI
/// accepted a non-null value and all of its callers therefore expected `T`.
/// Supplying the solved signatures keeps null-receiver exception planning and
/// the emitted ABI on one representation decision.
pub fn plan_program_fallibility_with_signatures(
    program: &Program,
    call_graph: &CallGraph,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
) -> ProgramFallibilityPlan {
    plan_program_fallibility_with_cast_context(program, call_graph, facts, signatures, None)
}

pub(crate) fn plan_program_fallibility_with_cast_context(
    program: &Program,
    call_graph: &CallGraph,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
    representations: Option<&crate::representation::RepresentationPlan>,
) -> ProgramFallibilityPlan {
    let mut callables = call_graph
        .nodes()
        .iter()
        .cloned()
        .map(|key| {
            let fallibility = seed_callable_fallibility(&key, facts);
            (key, fallibility)
        })
        .collect::<HashMap<_, _>>();
    let callback_plan;
    let callbacks = if let Some(signatures) = signatures {
        signatures.source_callback_plan()
    } else {
        callback_plan = crate::source_callbacks::SourceCallbackPlan::build(program, facts);
        &callback_plan
    };
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    for ty in types {
        if ty.kind == TypeKind::Enum {
            let key = CallableKey::method_identity(
                ty.name.clone(),
                crate::java_naming::ENUM_VALUE_OF_RESULT_METHOD,
            );
            let entry = callables.entry(key).or_default();
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::EnumLookupThrow);
        }
        for member in &ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !method
                .modifiers
                .iter()
                .any(|m| matches!(m, Modifier::Public | Modifier::Protected))
            {
                continue;
            }
            for index in 0..method.params.len() {
                if !callbacks.is_inline(ty, method, index) {
                    continue;
                }
                let key = CallableKey::CallbackParameter {
                    owner: Box::new(CallableKey::source_method_for_type(ty, method)),
                    index,
                };
                // Retained callbacks use a separate owning/value ABI. The
                // borrowed FnMut optimization is the boundary handled here.
                if callbacks.retains_callback(&key) {
                    continue;
                }
                if let Some(entry) = callables.get_mut(&key) {
                    entry.returns_result = true;
                    entry
                        .reasons
                        .push(FallibilityReason::OpenCallbackParameterThrow);
                }
            }
        }
    }
    // Declared throw facts are recorded under the Java name as well as the Rust
    // name. Seeding both here means the fallibility plan is the single store;
    // `MethodExceptionPlan` no longer needs its own parallel lookup.
    for (key, _) in declared_throw_callables(program, facts) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::MethodThrowFact);
        }
    }
    // Integer `/` and `%` throw ArithmeticException on a zero divisor, and that
    // is an unchecked exception, so it propagates out of the method like any
    // other throw. Seeding it here closes it globally: the fixpoint then makes
    // every transitive caller fallible instead of the divide site panicking.
    for key in integer_div_rem_callables(program) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::IntegerDivRemThrow);
        }
    }
    for key in checked_native_arithmetic_callables(program) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry
                .reasons
                .push(FallibilityReason::CheckedNativeArithmeticThrow);
        }
    }
    for key in nullable_receiver_throw_callables(program, signatures, facts) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::NullReceiverThrow);
        }
    }
    for key in shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| {
            matches!(
                expr,
                Expr::Call { .. } | Expr::New { .. } | Expr::ArrayAccess(..) | Expr::Cast { .. }
            )
        },
        Some(|body| !switch_selector_sites(body).is_empty()),
        block_has_unhandled_shaped_call_null_check,
    ) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        if !entry
            .reasons
            .contains(&FallibilityReason::NullReceiverThrow)
        {
            entry.reasons.push(FallibilityReason::NullReceiverThrow);
        }
    }
    for key in shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| {
            matches!(expr, Expr::Call { name, args, .. }
            if args.is_empty() && matches!(name.as_str(), "hashCode" | "hash_code"))
        },
        None,
        block_has_unhandled_checked_hash,
    ) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry
            .reasons
            .push(FallibilityReason::VirtualObjectDispatchThrow);
    }
    for key in shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| matches!(expr, Expr::Call { name, .. } if matches!(name.as_str(), "requireNonNull" | "require_non_null")),
        None,
        block_has_unhandled_checked_identity_null,
    ) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry.reasons.push(FallibilityReason::NullCheckThrow);
    }
    for key in checked_cast_throw_callables(program, facts, signatures, representations) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry.reasons.push(FallibilityReason::ClassCastThrow);
    }
    for key in array_bounds_throw_callables(program) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry.reasons.push(FallibilityReason::ArrayBoundsThrow);
    }
    for key in shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| matches!(expr, Expr::Call { name, .. } if matches!(name.as_str(), "copyOf" | "copy_of" | "copyOfRange" | "copy_of_range")),
        None,
        block_has_unhandled_native_array_copy_effect,
    ) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry.reasons.push(FallibilityReason::NativeArrayCopyThrow);
    }
    if signatures.is_some() {
        for key in shaped_immediate_effect_callables(
            program,
            facts,
            signatures,
            representations,
            |expr| {
                matches!(expr, Expr::Call { receiver: Some(_), name, args, .. }
                if args.len() == 1 && matches!(name.as_str(), "add" | "remove" | "contains"))
            },
            None,
            block_has_unhandled_semantic_set_effect,
        ) {
            let entry = callables.entry(key).or_default();
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::SemanticSetKeyThrow);
        }
        for key in shaped_immediate_effect_callables(
            program,
            facts,
            signatures,
            representations,
            |expr| {
                matches!(
                    expr,
                    Expr::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                ) || matches!(expr, Expr::New { ty, args, body: None, .. }
                    if args.len() == 1 && crate::effective_facts::EffectiveFacts::new(facts)
                        .native_collection_requires_runtime_policy(ty))
                    || matches!(expr, Expr::Call { receiver: Some(_), name, .. }
                    if matches!(name.as_str(), "get" | "getFirst" | "get_first" | "getLast" | "get_last" | "element" | "toString" | "to_string" | "valueOf" | "value_of" | "format" | "print" | "println")
                        || crate::stdlib::native_collection_method_name_mutates_receiver(name))
            },
            None,
            block_has_unhandled_retained_native_effect,
        ) {
            let entry = callables.entry(key).or_default();
            entry.returns_result = true;
            entry
                .reasons
                .push(FallibilityReason::RetainedNativeCollectionThrow);
        }
        for key in shaped_immediate_effect_callables(
            program,
            facts,
            signatures,
            representations,
            |_| false,
            Some(|body| !enhanced_for_iterable_sites(body).is_empty()),
            block_has_unhandled_retained_iteration,
        ) {
            let entry = callables.entry(key).or_default();
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::IteratorProtocolThrow);
        }
        for key in shaped_immediate_effect_callables(
            program,
            facts,
            signatures,
            representations,
            |expr| matches!(expr, Expr::Call { name, .. } if matches!(name.as_str(), "stream" | "parallelStream" | "parallel_stream")),
            None,
            block_has_unhandled_retained_stream,
        ) {
            let entry = callables.entry(key).or_default();
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::IteratorProtocolThrow);
        }
    }
    for key in shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| {
            matches!(expr, Expr::Call { receiver: Some(_), name, args, .. }
            if (args.is_empty() && matches!(name.as_str(), "toString" | "to_string"))
                || (args.len() == 1 && name == "equals") || name == "format")
        },
        None,
        block_has_unhandled_virtual_object_dispatch,
    ) {
        let entry = callables.entry(key).or_default();
        entry.returns_result = true;
        entry
            .reasons
            .push(FallibilityReason::VirtualObjectDispatchThrow);
    }
    for key in direct_throw_callables(program, facts) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::DirectThrow);
        }
    }
    // Synthetic callable bodies reuse the same intrinsic-effect policies as
    // source methods, under their own identities rather than their creator's.
    for (key, params, body) in call_graph.synthetic_callable_bodies() {
        let nullable_params = nullable_reference_param_names(params);
        let mut cast_env = crate::expr_shape::ShapeEnv::new();
        for param in params {
            let shape = crate::shape_projection::project_java_value_shape(
                representations,
                param.ty.clone(),
            );
            cast_env.insert_param_shape_with_java_type(
                param.name.clone(),
                shape.clone(),
                param.ty.clone(),
            );
            cast_env.insert_param_shape_with_java_type(
                param.source_name.clone(),
                shape,
                param.ty.clone(),
            );
        }
        let fallback_signatures = crate::signature::SignaturePlan::default();
        let reasons = [
            (
                block_has_unhandled_virtual_object_dispatch(
                    body,
                    &cast_env,
                    crate::expr_shape::ExprShapeContext::new(
                        signatures,
                        representations,
                        Some(facts),
                    ),
                    &[],
                    signatures.unwrap_or(&fallback_signatures),
                ),
                FallibilityReason::VirtualObjectDispatchThrow,
            ),
            (
                block_has_unhandled_native_array_copy_effect(
                    body,
                    &cast_env,
                    crate::expr_shape::ExprShapeContext::new(
                        signatures,
                        representations,
                        Some(facts),
                    ),
                    &[],
                    signatures.unwrap_or(&fallback_signatures),
                ),
                FallibilityReason::NativeArrayCopyThrow,
            ),
            (
                block_has_unhandled_array_bounds(body),
                FallibilityReason::ArrayBoundsThrow,
            ),
            (
                block_has_unhandled_checked_cast(
                    body,
                    &cast_env,
                    crate::expr_shape::ExprShapeContext::new(
                        signatures,
                        representations,
                        Some(facts),
                    ),
                    &[],
                    signatures.unwrap_or(&fallback_signatures),
                ),
                FallibilityReason::ClassCastThrow,
            ),
            (
                block_has_unhandled_integer_div_rem(body),
                FallibilityReason::IntegerDivRemThrow,
            ),
            (
                block_has_unhandled_checked_native_arithmetic(body),
                FallibilityReason::CheckedNativeArithmeticThrow,
            ),
            (
                !nullable_params.is_empty()
                    && block_has_unhandled_nullable_receiver_deref(body, &nullable_params),
                FallibilityReason::NullReceiverThrow,
            ),
            (
                block_has_unhandled_checked_identity_null(
                    body,
                    &cast_env,
                    crate::expr_shape::ExprShapeContext::new(
                        signatures,
                        representations,
                        Some(facts),
                    ),
                    &[],
                    signatures.unwrap_or(&fallback_signatures),
                ),
                FallibilityReason::NullCheckThrow,
            ),
            (
                block_has_unhandled_direct_throw(body, facts, &[]),
                FallibilityReason::DirectThrow,
            ),
        ];
        for (applies, reason) in reasons {
            if applies {
                let entry = callables.entry(key.clone()).or_default();
                entry.returns_result = true;
                if !entry.reasons.contains(&reason) {
                    entry.reasons.push(reason);
                }
            }
        }
    }
    let override_groups = override_groups_for_program(program);
    let mut solve_iterations = 0usize;
    loop {
        solve_iterations += 1;
        let mut changed = false;
        for edge in call_graph.edges() {
            match edge.kind() {
                crate::usage::CallGraphEdgeKind::HandledExceptionBoundary
                | crate::usage::CallGraphEdgeKind::AnonymousClassMethod => continue,
                crate::usage::CallGraphEdgeKind::AnonymousOverrideContract
                | crate::usage::CallGraphEdgeKind::CallbackContractEquality => {
                    // An anonymous implementation and its virtual base slot
                    // have one Rust return ABI. This is a contract equality,
                    // not a call from the constructing method into the body.
                    let returns_result = [edge.caller(), edge.callee()]
                        .into_iter()
                        .any(|key| callables.get(key).is_some_and(|f| f.returns_result));
                    if returns_result {
                        for key in [edge.caller(), edge.callee()] {
                            let entry = callables.entry(key.clone()).or_default();
                            if !entry.returns_result {
                                entry.returns_result = true;
                                entry
                                    .reasons
                                    .push(FallibilityReason::PropagatedFromOverride);
                                changed = true;
                            }
                        }
                    }
                    continue;
                }
                _ => {}
            }
            let callee_returns_result = callables
                .get(edge.callee())
                .is_some_and(|fallibility| fallibility.returns_result);
            if !callee_returns_result {
                continue;
            }
            let caller = callables.entry(edge.caller().clone()).or_default();
            if !caller.returns_result {
                caller.returns_result = true;
                caller.reasons.push(FallibilityReason::PropagatedFromCallee);
                changed = true;
            }
        }
        // A trait method and every implementation of it must share one Rust
        // signature, so fallibility has to be equalized across each override
        // set: if any implementation can throw, the declaration and all its
        // siblings must return `Result` too. Without this a call through an
        // interface cannot propagate, and the forwarding impl fails to compile.
        for group in &override_groups {
            if !group
                .iter()
                .any(|key| callables.get(key).is_some_and(|f| f.returns_result))
            {
                continue;
            }
            for key in group {
                let entry = callables.entry(key.clone()).or_default();
                if !entry.returns_result {
                    entry.returns_result = true;
                    entry
                        .reasons
                        .push(FallibilityReason::PropagatedFromOverride);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    ProgramFallibilityPlan {
        callables,
        solve_iterations,
    }
}

/// Callables with a declared throw fact under either their Rust or Java name.
fn declared_throw_callables(program: &Program, facts: &FactSet) -> Vec<(CallableKey, ())> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            let rust_name = source_method_rust_name(method);
            if facts.method_has_thrown_exceptions(&ty.name, &rust_name)
                || facts.method_has_thrown_exceptions(&ty.name, &method.name)
            {
                keys.push((CallableKey::source_method_for_type(ty, method), ()));
            }
        }
    }
    keys
}

/// Callables whose body contains an integer `/` or `%`, which can throw
/// ArithmeticException.
/// Whether any of these catch clauses would handle an ArithmeticException.
///
/// ArithmeticException is unchecked, so the existing
/// `HandledExceptionBoundary` edge — which only considers *checked* exceptions
/// — never clears it. Without this, a method that catches its own divide-by-zero
/// is still marked fallible and returns `Result` for an exception it handles.
fn catches_arithmetic_exception(catches: &[CatchClause]) -> bool {
    catches_intrinsic_exception(catches, "ArithmeticException")
}

/// Integer `/` or `%` that is *not* already handled by an enclosing catch.
fn block_has_unhandled_integer_div_rem(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_has_unhandled_integer_div_rem)
}

fn stmt_has_unhandled_integer_div_rem(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            // A catch that handles it stops the propagation, so the guarded
            // region contributes nothing. The catch and finally bodies are
            // outside the guard and still count.
            let guarded_counts = !catches_arithmetic_exception(catches);
            (guarded_counts
                && (resources.iter().any(stmt_has_unhandled_integer_div_rem)
                    || block_has_unhandled_integer_div_rem(body)))
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_integer_div_rem(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_integer_div_rem)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_integer_div_rem_expr(condition)
                || block_has_unhandled_integer_div_rem(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_integer_div_rem)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_has_integer_div_rem_expr(condition) || block_has_unhandled_integer_div_rem(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_has_unhandled_integer_div_rem)
                || condition
                    .as_ref()
                    .is_some_and(expr_has_integer_div_rem_expr)
                || update.iter().any(expr_has_integer_div_rem_expr)
                || block_has_unhandled_integer_div_rem(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_integer_div_rem_expr(iterable) || block_has_unhandled_integer_div_rem(body)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_integer_div_rem(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_integer_div_rem_expr(selector)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(switch_label_has_integer_div_rem_expr)
                        || case
                            .statements
                            .iter()
                            .any(stmt_has_unhandled_integer_div_rem)
                })
        }
        _ => stmt_has_integer_div_rem_expr(stmt),
    }
}

fn block_has_unhandled_integer_div_rem_with_env(block: &Block, env: &TypeEnv) -> bool {
    block_has_unhandled_expression_effect(
        block,
        env,
        &ExpressionEffect {
            expression: &expr_has_throwing_integer_div_rem_expr,
            handles: &catches_arithmetic_exception,
            learn: None,
        },
    )
}

/// Shared typed control-flow traversal for immediate expression effects.
/// Environments are borrowed until a local declaration requires a scoped copy.
pub(crate) struct ExpressionEffect<'a, E> {
    pub(crate) expression: &'a dyn Fn(&Expr, &E) -> bool,
    pub(crate) handles: &'a dyn Fn(&[CatchClause]) -> bool,
    pub(crate) learn: Option<&'a dyn Fn(&Stmt, &mut E)>,
}

fn catches_array_bounds_exception(catches: &[CatchClause]) -> bool {
    catches_intrinsic_exception(catches, "ArrayIndexOutOfBoundsException")
}

/// Intrinsic effects use the same JDK subtype closure as explicit throws and
/// the semantic exception model, rather than owning superclass answer lists.
fn catches_intrinsic_exception(catches: &[CatchClause], thrown: &str) -> bool {
    catches
        .iter()
        .flat_map(|catch| &catch.types)
        .any(|ty| crate::stdlib::is_known_java_subtype(thrown, &exception_class_name(ty)))
}

fn expr_has_immediate_array_access(expr: &Expr) -> bool {
    let mut found = false;
    crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
        found |= matches!(expr, Expr::ArrayAccess(..));
        if let Expr::New {
            body: Some(members),
            ..
        } = expr
        {
            // Object initialization executes now; its methods execute later.
            found |= members.iter().any(|member| match member {
                Member::Field(field) => field
                    .initializer
                    .as_ref()
                    .is_some_and(expr_has_immediate_array_access),
                _ => false,
            });
        }
    });
    found
}

fn block_has_unhandled_array_bounds(body: &Block) -> bool {
    block_has_unhandled_expression_effect(
        body,
        &(),
        &ExpressionEffect {
            expression: &|expr, _| expr_has_immediate_array_access(expr),
            handles: &catches_array_bounds_exception,
            learn: None,
        },
    )
}

fn array_bounds_throw_callables(program: &Program) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            let (key, body) = match member {
                Member::Method(method) => (
                    CallableKey::source_method_for_type(ty, method),
                    method.body.as_ref(),
                ),
                Member::Constructor(ctor) => (
                    CallableKey::source_constructor_for_type(ty, ctor),
                    ctor.body.as_ref(),
                ),
                _ => continue,
            };
            if body.is_some_and(block_has_unhandled_array_bounds) {
                keys.push(key);
            }
        }
    }
    keys
}

pub(crate) fn block_has_unhandled_expression_effect<E: Clone>(
    block: &Block,
    env: &E,
    effect: &ExpressionEffect<'_, E>,
) -> bool {
    let mut frame = std::borrow::Cow::Borrowed(env);
    statements_have_unhandled_expression_effect(&block.statements, &mut frame, effect)
}

fn statements_have_unhandled_expression_effect<E: Clone>(
    statements: &[Stmt],
    frame: &mut std::borrow::Cow<'_, E>,
    effect: &ExpressionEffect<'_, E>,
) -> bool {
    for stmt in statements {
        if stmt_has_unhandled_expression_effect(stmt, frame.as_ref(), effect) {
            return true;
        }
        if matches!(stmt, Stmt::VarDecl { .. }) {
            if let Some(learn) = effect.learn {
                learn(stmt, frame.to_mut());
            }
        }
    }
    false
}

fn stmt_has_unhandled_expression_effect<E: Clone>(
    stmt: &Stmt,
    env: &E,
    effect: &ExpressionEffect<'_, E>,
) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let guarded_counts = !(effect.handles)(catches);
            let mut resource_frame = std::borrow::Cow::Borrowed(env);
            (guarded_counts
                && (statements_have_unhandled_expression_effect(
                    resources,
                    &mut resource_frame,
                    effect,
                ) || block_has_unhandled_expression_effect(
                    body,
                    resource_frame.as_ref(),
                    effect,
                )))
                || catches.iter().any(|catch| {
                    let mut catch_frame = std::borrow::Cow::Borrowed(env);
                    if let (Some(learn), Some(ty)) = (effect.learn, catch.types.first()) {
                        learn(
                            &Stmt::VarDecl {
                                ty: ty.clone(),
                                name: catch.name.clone(),
                                source_name: catch.source_name.clone(),
                                initializer: None,
                                binding: catch.binding.clone(),
                            },
                            catch_frame.to_mut(),
                        );
                    }
                    block_has_unhandled_expression_effect(&catch.body, catch_frame.as_ref(), effect)
                })
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_expression_effect(block, env, effect))
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| (effect.expression)(expr, env)),
        Stmt::Assign { target, value } => {
            (effect.expression)(target, env) || (effect.expression)(value, env)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => (effect.expression)(expr, env),
        Stmt::Return(expr) => expr
            .as_ref()
            .is_some_and(|expr| (effect.expression)(expr, env)),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            (effect.expression)(condition, env)
                || block_has_unhandled_expression_effect(then_block, env, effect)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_expression_effect(block, env, effect))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            (effect.expression)(condition, env)
                || block_has_unhandled_expression_effect(body, env, effect)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut frame = std::borrow::Cow::Borrowed(env);
            statements_have_unhandled_expression_effect(init, &mut frame, effect)
                || condition
                    .as_ref()
                    .is_some_and(|expr| (effect.expression)(expr, frame.as_ref()))
                || update
                    .iter()
                    .any(|expr| (effect.expression)(expr, frame.as_ref()))
                || block_has_unhandled_expression_effect(body, frame.as_ref(), effect)
        }
        Stmt::EnhancedFor {
            iterable,
            body,
            var_type,
            var_name,
            source_var_name,
            binding,
            ..
        } => {
            if (effect.expression)(iterable, env) {
                return true;
            }
            let mut frame = std::borrow::Cow::Borrowed(env);
            if let Some(learn) = effect.learn {
                learn(
                    &Stmt::VarDecl {
                        ty: var_type.clone(),
                        name: var_name.clone(),
                        source_name: source_var_name.clone(),
                        initializer: None,
                        binding: binding.clone(),
                    },
                    frame.to_mut(),
                );
            }
            block_has_unhandled_expression_effect(body, frame.as_ref(), effect)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_expression_effect(body, env, effect)
        }
        Stmt::Switch { selector, cases } => {
            // Java case groups share the switch block's local scope. Learn
            // declarations in source order without leaking them outside it.
            let mut frame = std::borrow::Cow::Borrowed(env);
            (effect.expression)(selector, env)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => (effect.expression)(expr, frame.as_ref()),
                        SwitchLabel::Default => false,
                    }) || statements_have_unhandled_expression_effect(
                        &case.statements,
                        &mut frame,
                        effect,
                    )
                })
        }
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(|expr| (effect.expression)(expr, env))
        }
        // A local/anonymous type owns a separate type environment. The call
        // graph seeds its callable independently, so it must not contaminate
        // the enclosing callable here.
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn callable_integer_div_rem_type_env(ty: &TypeDecl, params: &[crate::java_ir::Param]) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
            env.insert_field(crate::parser::to_snake_case(&field.name), field.ty.clone());
            env.insert_type_field(ty.name.clone(), field.name.clone(), field.ty.clone());
        }
    }
    for param in params {
        env.insert_param(param.name.clone(), param.ty.clone());
        if param.source_name != param.name {
            env.insert_param(param.source_name.clone(), param.ty.clone());
        }
    }
    env
}

fn integer_div_rem_callables(program: &Program) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    let env = callable_integer_div_rem_type_env(ty, &method.params);
                    if method.body.as_ref().is_some_and(|body| {
                        block_has_unhandled_integer_div_rem_with_env(body, &env)
                    }) {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    let env = callable_integer_div_rem_type_env(ty, &constructor.params);
                    if constructor.body.as_ref().is_some_and(|body| {
                        block_has_unhandled_integer_div_rem_with_env(body, &env)
                    }) {
                        keys.push(CallableKey::source_constructor_for_type(ty, constructor));
                    }
                }
                _ => {}
            }
        }
    }
    keys
}

fn checked_native_arithmetic_callables(program: &Program) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if method
                        .body
                        .as_ref()
                        .is_some_and(block_has_unhandled_checked_native_arithmetic)
                    {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    if constructor
                        .body
                        .as_ref()
                        .is_some_and(block_has_unhandled_checked_native_arithmetic)
                    {
                        keys.push(CallableKey::source_constructor_for_type(ty, constructor));
                    }
                }
                _ => {}
            }
        }
    }
    keys
}

fn block_has_unhandled_checked_native_arithmetic(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_has_unhandled_checked_native_arithmetic)
}

fn stmt_has_unhandled_checked_native_arithmetic(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let guarded_counts = !catches_arithmetic_exception(catches);
            (guarded_counts
                && (resources
                    .iter()
                    .any(stmt_has_unhandled_checked_native_arithmetic)
                    || block_has_unhandled_checked_native_arithmetic(body)))
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_checked_native_arithmetic(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_checked_native_arithmetic)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_checked_native_arithmetic_call(condition)
                || block_has_unhandled_checked_native_arithmetic(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_checked_native_arithmetic)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_has_checked_native_arithmetic_call(condition)
                || block_has_unhandled_checked_native_arithmetic(body)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(stmt_has_unhandled_checked_native_arithmetic)
                || condition
                    .as_ref()
                    .is_some_and(expr_has_checked_native_arithmetic_call)
                || update.iter().any(expr_has_checked_native_arithmetic_call)
                || block_has_unhandled_checked_native_arithmetic(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_checked_native_arithmetic_call(iterable)
                || block_has_unhandled_checked_native_arithmetic(body)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_checked_native_arithmetic(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_checked_native_arithmetic_call(selector)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(switch_label_has_checked_native_arithmetic_call)
                        || case
                            .statements
                            .iter()
                            .any(stmt_has_unhandled_checked_native_arithmetic)
                })
        }
        _ => stmt_has_checked_native_arithmetic_call(stmt),
    }
}

fn stmt_has_checked_native_arithmetic_call(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(expr_has_checked_native_arithmetic_call),
        Stmt::Assign { target, value } => {
            expr_has_checked_native_arithmetic_call(target)
                || expr_has_checked_native_arithmetic_call(value)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
            expr_has_checked_native_arithmetic_call(expr)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
        Stmt::Throw(expr) => expr_has_checked_native_arithmetic_call(expr),
        _ => false,
    }
}

fn switch_label_has_checked_native_arithmetic_call(label: &crate::java_ir::SwitchLabel) -> bool {
    match label {
        crate::java_ir::SwitchLabel::Default => false,
        crate::java_ir::SwitchLabel::Expr(expr) => expr_has_checked_native_arithmetic_call(expr),
    }
}

fn expr_has_checked_native_arithmetic_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            call_is_checked_native_arithmetic(receiver.as_deref(), name, args.len())
                || receiver
                    .as_ref()
                    .is_some_and(|receiver| expr_has_checked_native_arithmetic_call(receiver))
                || args.iter().any(expr_has_checked_native_arithmetic_call)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_checked_native_arithmetic_call(condition)
                || expr_has_checked_native_arithmetic_call(then_expr)
                || expr_has_checked_native_arithmetic_call(else_expr)
        }
        Expr::Switch { selector, cases } => {
            expr_has_checked_native_arithmetic_call(selector)
                || cases.iter().any(|case| {
                    expr_has_checked_native_arithmetic_call(&case.value)
                        || case
                            .labels
                            .iter()
                            .any(switch_label_has_checked_native_arithmetic_call)
                })
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_has_checked_native_arithmetic_call(receiver)
        }
        Expr::Binary { left, right, .. } => {
            expr_has_checked_native_arithmetic_call(left)
                || expr_has_checked_native_arithmetic_call(right)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_has_checked_native_arithmetic_call(qualifier))
                || args.iter().any(expr_has_checked_native_arithmetic_call)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(block_has_checked_native_arithmetic_call),
                        Member::Constructor(constructor) => constructor
                            .body
                            .as_ref()
                            .is_some_and(block_has_checked_native_arithmetic_call),
                        Member::Field(field) => field
                            .initializer
                            .as_ref()
                            .is_some_and(expr_has_checked_native_arithmetic_call),
                        Member::Type(nested) => type_has_checked_native_arithmetic_call(nested),
                        Member::Unsupported { .. } => false,
                    })
                })
        }
        Expr::ArrayInitializer(args) => args.iter().any(expr_has_checked_native_arithmetic_call),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(expr_has_checked_native_arithmetic_call)
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(expr_has_checked_native_arithmetic_call))
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super => false,
    }
}

fn call_is_checked_native_arithmetic(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
) -> bool {
    (matches!(name, "toIntExact" | "to_int_exact") && arg_count == 1
        || matches!(
            name,
            "floorDiv"
                | "floor_div"
                | "floorMod"
                | "floor_mod"
                | "multiplyExact"
                | "multiply_exact"
        ) && arg_count == 2)
        && receiver.is_none_or(expr_is_math_static_owner)
        || matches!(
            name,
            "divideUnsigned" | "divide_unsigned" | "remainderUnsigned" | "remainder_unsigned"
        ) && arg_count == 2
            && receiver.is_none_or(|receiver| match receiver {
                Expr::Var(owner) => matches!(owner.rsplit('.').next(), Some("Integer" | "Long")),
                Expr::FieldAccess(_, owner) => matches!(owner.as_str(), "Integer" | "Long"),
                _ => false,
            })
}

fn expr_is_math_static_owner(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => matches!(name.as_str(), "Math" | "StrictMath"),
        Expr::FieldAccess(owner, field) => {
            if matches!(field.as_str(), "Math" | "StrictMath") {
                return true;
            }
            expr_is_math_static_owner(owner)
        }
        _ => false,
    }
}

fn catches_class_cast_exception(catches: &[CatchClause]) -> bool {
    catches_intrinsic_exception(catches, "ClassCastException")
}

fn expr_has_checked_cast(
    expr: &Expr,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
) -> bool {
    let mut found = false;
    crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
        if found {
            return;
        }
        if let Expr::Cast { ty, expr } = expr {
            if !matches!(ty, JavaType::Primitive(_) | JavaType::Void)
                && !matches!(expr.as_ref(), Expr::Literal(crate::java_ir::Literal::Null))
            {
                let plan = crate::conversion_plan::plan_cast_conversion(expr, ty, env, ctx);
                found = plan.coercion().unsupported_diagnostics().is_empty()
                    && plan.coercion().may_throw_class_cast();
            }
        }
        // Anonymous methods execute later, but their field initializers run
        // immediately when the object is constructed.
        if let Expr::New {
            body: Some(members),
            ..
        } = expr
        {
            found |= members.iter().any(|member| match member {
                Member::Field(field) => field
                    .initializer
                    .as_ref()
                    .is_some_and(|expr| expr_has_checked_cast(expr, env, ctx)),
                _ => false,
            });
        }
    });
    found
}

fn checked_cast_throw_callables(
    program: &Program,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
    representations: Option<&crate::representation::RepresentationPlan>,
) -> Vec<CallableKey> {
    shaped_immediate_effect_callables(
        program,
        facts,
        signatures,
        representations,
        |expr| matches!(expr, Expr::Cast { .. }),
        None,
        block_has_unhandled_checked_cast,
    )
}

fn shaped_immediate_effect_callables(
    program: &Program,
    facts: &FactSet,
    signatures: Option<&crate::signature::SignaturePlan>,
    representations: Option<&crate::representation::RepresentationPlan>,
    candidate: impl Fn(&Expr) -> bool,
    block_candidate: Option<fn(&Block) -> bool>,
    check: fn(
        &Block,
        &crate::expr_shape::ShapeEnv,
        crate::expr_shape::ExprShapeContext<'_>,
        &[crate::signature::LocalPlan],
        &crate::signature::SignaturePlan,
    ) -> bool,
) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    let fallback_signatures = crate::signature::SignaturePlan::default();
    let signature_plan = signatures.unwrap_or(&fallback_signatures);
    let external_index = facts.external_lookup_index();
    let ctx = crate::expr_shape::ExprShapeContext::new(signatures, representations, Some(facts))
        .with_external_index(&external_index);
    for ty in types {
        for member in &ty.members {
            let body = match member {
                Member::Method(method) => method.body.as_ref(),
                Member::Constructor(ctor) => ctor.body.as_ref(),
                _ => None,
            };
            let Some(body) = body else {
                continue;
            };
            let mut contains_candidate = block_candidate.is_some_and(|candidate| candidate(body));
            crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
                contains_candidate |= candidate(expr);
            });
            if !contains_candidate {
                continue;
            }
            let (key, params, body, solved_env, locals) = match member {
                Member::Method(method) => (
                    CallableKey::source_method_for_type(ty, method),
                    method.params.as_slice(),
                    method.body.as_ref(),
                    signatures.and_then(|plan| {
                        plan.method_signature(ty, method).map(|signature| {
                            crate::expr_shape::shape_env_for_method(ty, method, signature, plan)
                        })
                    }),
                    signature_plan.method_local_shapes_or_empty(ty, method),
                ),
                Member::Constructor(ctor) => (
                    CallableKey::source_constructor_for_type(ty, ctor),
                    ctor.params.as_slice(),
                    ctor.body.as_ref(),
                    signatures.and_then(|plan| {
                        plan.constructor_signature(ty, ctor).map(|signature| {
                            crate::expr_shape::shape_env_for_constructor(ty, ctor, signature, plan)
                        })
                    }),
                    signatures
                        .and_then(|plan| plan.constructor_signature(ty, ctor))
                        .map(|signature| signature.locals())
                        .unwrap_or(&[]),
                ),
                _ => continue,
            };
            let Some(body) = body else {
                continue;
            };
            let mut env = solved_env.unwrap_or_else(|| {
                let mut env = crate::expr_shape::ShapeEnv::new();
                if ty.kind == crate::java_ir::TypeKind::Interface
                    && !matches!(member, Member::Method(method) if method.modifiers.contains(&Modifier::Static)) {
                    env.bind_this_type(ty);
                }
                for (index, name) in ty.type_params.iter().enumerate() {
                    env.insert_type_param_declaration(
                        name.clone(),
                        ty.type_param_bounds.get(index).cloned().flatten(),
                    );
                }
                let generic_params = match member {
                    Member::Method(method) => {
                        Some((&method.type_params, &method.type_param_bounds))
                    }
                    Member::Constructor(ctor) => Some((&ctor.type_params, &ctor.type_param_bounds)),
                    _ => None,
                };
                if let Some((names, bounds)) = generic_params {
                    for (index, name) in names.iter().enumerate() {
                        env.insert_type_param_declaration(
                            name.clone(),
                            bounds.get(index).cloned().flatten(),
                        );
                    }
                }
                for field in &ty.members {
                    if let Member::Field(field) = field {
                        env.insert_field_shape(
                            field.name.clone(),
                            crate::shape_projection::project_java_value_shape(
                                representations,
                                field.ty.clone(),
                            ),
                        );
                    }
                }
                for param in params {
                    let shape = crate::shape_projection::project_java_value_shape(
                        representations,
                        param.ty.clone(),
                    );
                    env.insert_param_shape_with_java_type(
                        param.name.clone(),
                        shape.clone(),
                        param.ty.clone(),
                    );
                    env.insert_param_shape_with_java_type(
                        param.source_name.clone(),
                        shape,
                        param.ty.clone(),
                    );
                }
                env
            });
            // Effect analysis uses the same inherited field slots as lowering,
            // instantiated in the receiver's declaration frame. Own fields
            // already in the environment retain Java hiding precedence.
            let mut seen_fields = ty
                .members
                .iter()
                .filter_map(|member| match member {
                    Member::Field(field) => Some(field.name.clone()),
                    _ => None,
                })
                .collect::<HashSet<_>>();
            for inherited in
                crate::inherited_members::inherited_instance_field_refs_for_type(ty, program)
            {
                let field = inherited.field();
                if !seen_fields.insert(field.name.clone()) {
                    continue;
                }
                let Some(declaring_ty) = crate::inherited_members::find_type_decl_for_owner_context(
                    program,
                    &ty.name,
                    inherited.declaring_type(),
                ) else {
                    continue;
                };
                let bindings = crate::inherited_members::source_ancestor_type_arguments(
                    program,
                    ty,
                    declaring_ty,
                )
                .unwrap_or_default();
                let java_ty =
                    crate::resolve::substitute_method_type_variables(&field.ty, &bindings);
                let shape = signature_plan
                    .field_shape(inherited.declaring_type(), &field.name)
                    .map(|shape| {
                        crate::shape_projection::specialize_slot_shape_java_type(shape, &java_ty)
                    })
                    .unwrap_or_else(|| {
                        crate::shape_projection::project_java_value_shape(representations, java_ty)
                    });
                env.insert_field_shape(field.name.clone(), shape);
            }
            // Enclosing-instance fields are lexical bindings too. Use the
            // same stored slots and hiding policy as lowering; otherwise a
            // nullable outer receiver gets an infallible signature and its
            // required null projection cannot propagate a Java exception.
            if let Some(outer) = facts.captures.outer_this_type_for_type(&ty.name)
                && let Some(outer_decl) = crate::inherited_members::find_type_decl_for_owner_context(
                    program, &ty.name, outer,
                )
            {
                let mut outer_env =
                    crate::expr_shape::shape_env_for_type(outer_decl, signature_plan);
                crate::expr_shape::seed_inherited_instance_fields(
                    &mut outer_env,
                    outer_decl,
                    signature_plan,
                    program,
                );
                env.seed_lexical_outer_instance_fields(&outer_env);
            }
            if check(body, &env, ctx, locals, signature_plan) {
                keys.push(key);
            }
        }
    }
    keys
}

/// Sites identify the implicit operation without conflating an ordinary read
/// of the same variable elsewhere in the body. Addresses never escape the
/// immediate immutable-body traversal or become cache keys.
pub(crate) fn enhanced_for_iterable_sites(body: &Block) -> HashSet<*const Expr> {
    let mut sites = HashSet::new();
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
        if let Stmt::EnhancedFor { iterable, .. } = stmt {
            sites.insert(iterable as *const Expr);
        }
    });
    sites
}

/// A switch performs a null check even when its selector is just a variable
/// and its body has no method call. Addresses identify only this body walk.
fn switch_selector_sites(body: &Block) -> HashSet<*const Expr> {
    let mut sites = HashSet::new();
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
        if let Stmt::Switch { selector, .. } = stmt {
            sites.insert(selector as *const Expr);
        }
    });
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if let Expr::Switch { selector, .. } = expr {
            sites.insert(selector.as_ref() as *const Expr);
        }
    });
    sites
}

fn block_has_unhandled_checked_hash(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
            found |=
                crate::java_hash_plan::plan_java_hash_call(expr, env.type_env(), facts).is_some();
        });
        found
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &|catches| catches_intrinsic_exception(catches, "Throwable"),
            learn: Some(&learn),
        },
    )
}

fn block_has_unhandled_virtual_object_dispatch(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
            if crate::java_append_plan::plan_object_append(expr, env.type_env(), facts).is_some() {
                found = true;
            }
            let Expr::Call {
                receiver: Some(receiver),
                name,
                args,
                resolved,
                ..
            } = expr
            else {
                return;
            };
            found |= matches!(
                crate::call_plan::plan_call_target_with_facts_and_arity(
                    Some(receiver),
                    name,
                    args.len(),
                    resolved.as_ref(),
                    facts,
                    env.type_env(),
                ),
                crate::call_plan::CallTargetPlan::NativeJavaInstance {
                    method: crate::call_plan::NativeJavaInstanceMethod::VirtualObjectToString
                        | crate::call_plan::NativeJavaInstanceMethod::SourceObjectToString
                        | crate::call_plan::NativeJavaInstanceMethod::VirtualObjectEquals,
                    ..
                } | crate::call_plan::CallTargetPlan::NativeJavaStatic {
                    method: crate::call_plan::NativeJavaStaticMethod::StringFormat,
                }
            );
        });
        found
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &|catches| catches_intrinsic_exception(catches, "Throwable"),
            learn: Some(&learn),
        },
    )
}

fn block_has_unhandled_retained_iteration(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let sites = enhanced_for_iterable_sites(body);
    let mut unboxing_sites = HashSet::new();
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
        if let Stmt::EnhancedFor {
            var_type: JavaType::Primitive(_),
            iterable,
            ..
        } = stmt
        {
            unboxing_sites.insert(iterable as *const Expr);
        }
    });
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    for thrown in [
        "ConcurrentModificationException",
        "NoSuchElementException",
        "NullPointerException",
    ] {
        let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
            if !sites.contains(&(expr as *const Expr)) {
                return false;
            }
            let shape = crate::expr_shape::infer_expr_shape_with_context(expr, env, ctx);
            let Some(element) =
                crate::call_plan::retained_native_iteration_element_type(&shape, facts)
            else {
                return false;
            };
            thrown != "NullPointerException"
                || shape.optional_element_type().is_some()
                || (unboxing_sites.contains(&(expr as *const Expr))
                    && crate::types::optional_element_type(&element).is_some())
        };
        let handles = |catches: &[CatchClause]| catches_intrinsic_exception(catches, thrown);
        if block_has_unhandled_expression_effect(
            body,
            env,
            &ExpressionEffect {
                expression: &expression,
                handles: &handles,
                learn: Some(&learn),
            },
        ) {
            return true;
        }
    }
    false
}

fn block_has_unhandled_retained_stream(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    [
        "ConcurrentModificationException",
        "NoSuchElementException",
        "NullPointerException",
    ]
    .into_iter()
    .any(|thrown| {
        let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
            let mut found = false;
            crate::stream_shape::visit_retained_stream_sources(
                expr,
                env,
                ctx,
                |_, parent, consumed| {
                    found |= if thrown == "NullPointerException" {
                        parent.optional_element_type().is_some()
                    } else {
                        consumed
                    };
                },
            );
            found
        };
        block_has_unhandled_expression_effect(
            body,
            env,
            &ExpressionEffect {
                expression: &expression,
                handles: &|catches| catches_intrinsic_exception(catches, thrown),
                learn: Some(&learn),
            },
        )
    })
}

fn block_has_unhandled_semantic_set_effect(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let Some(representations) = ctx.representations() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
            if found {
                return;
            }
            let Expr::Call {
                receiver: Some(receiver),
                name,
                args,
                ..
            } = expr
            else {
                return;
            };
            if args.len() != 1 || !matches!(name.as_str(), "add" | "remove" | "contains") {
                return;
            }
            let shape = crate::expr_shape::infer_expr_shape_with_context(receiver, env, ctx);
            if shape.native_allocation.shared() {
                return;
            }
            let Some(crate::call_plan::CallTargetPlan::NativeCollectionMethod { method, .. }) =
                crate::call_plan::plan_native_collection_receiver_call_for_type(
                    receiver,
                    name,
                    Some(args),
                    &shape.java_ty,
                    env.type_env(),
                )
            else {
                return;
            };
            found = crate::call_plan::native_set_uses_checked_key_operation(
                method,
                &shape.java_ty,
                representations,
            );
        });
        found
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &|catches| catches_intrinsic_exception(catches, "RuntimeException"),
            learn: Some(&learn),
        },
    )
}

fn block_has_unhandled_native_array_copy_effect(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    use crate::call_plan::{CallTargetPlan, NativeCollectionStaticMethod};
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    for thrown in [
        "NegativeArraySizeException",
        "ArrayIndexOutOfBoundsException",
        "IllegalArgumentException",
        "NullPointerException",
    ] {
        let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
            let mut found = false;
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
                let Expr::Call {
                    receiver,
                    name,
                    args,
                    resolved,
                    ..
                } = expr
                else {
                    return;
                };
                let Some(CallTargetPlan::NativeCollectionStatic { method }) =
                    crate::call_plan::plan_native_collection_static_call(
                        receiver.as_deref(),
                        name,
                        resolved.as_ref(),
                        env.type_env(),
                        facts,
                    )
                else {
                    return;
                };
                let applicable = match method {
                    NativeCollectionStaticMethod::ArraysCopyOf if args.len() == 2 => matches!(
                        thrown,
                        "NegativeArraySizeException" | "NullPointerException"
                    ),
                    NativeCollectionStaticMethod::ArraysCopyOfRange if args.len() == 3 => {
                        thrown != "NegativeArraySizeException"
                    }
                    _ => false,
                };
                found |= applicable;
            });
            found
        };
        let handles = |catches: &[CatchClause]| catches_intrinsic_exception(catches, thrown);
        if block_has_unhandled_expression_effect(
            body,
            env,
            &ExpressionEffect {
                expression: &expression,
                handles: &handles,
                learn: Some(&learn),
            },
        ) {
            return true;
        }
    }
    false
}

fn block_has_unhandled_retained_native_effect(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    use crate::call_plan::{CallTargetPlan, NativeCollectionMethod};
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    for thrown in [
        "UnsupportedOperationException",
        "IndexOutOfBoundsException",
        "RuntimeException",
        "NoSuchElementException",
        "NullPointerException",
    ] {
        let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
            let mut found = false;
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
                if found {
                    return;
                }
                if thrown == "RuntimeException" {
                    found = crate::conversion_plan::implicit_java_display_operands(
                        expr,
                        env.type_env(),
                        facts,
                    )
                    .into_iter()
                    .any(|operand| {
                        let shape =
                            crate::expr_shape::infer_expr_shape_with_context(operand, env, ctx);
                        crate::conversion_plan::native_collection_display_kind(&shape, facts)
                            .is_some()
                            || env
                                .type_env()
                                .is_unbounded_object_type_parameter(&shape.java_ty)
                    });
                    if found {
                        return;
                    }
                }
                if thrown == "NullPointerException"
                    && let Expr::New {
                        ty,
                        args,
                        body: None,
                        ..
                    } = expr
                    && crate::effective_facts::EffectiveFacts::new(facts)
                        .native_collection_requires_runtime_policy(ty)
                    && matches!(
                        crate::call_plan::plan_native_collection_constructor_for_args(
                            ty,
                            args,
                            env.type_env(),
                            facts
                        ),
                        Some(crate::call_plan::NativeCollectionNewTarget::FromSource { .. })
                    )
                {
                    found = true;
                    return;
                }
                let Expr::Call {
                    receiver: Some(receiver),
                    name,
                    args,
                    resolved,
                    ..
                } = expr
                else {
                    return;
                };
                if thrown == "RuntimeException"
                    && !matches!(name.as_str(), "toString" | "to_string")
                {
                    return;
                }
                let shape = crate::expr_shape::infer_expr_shape_with_context(receiver, env, ctx);
                if crate::conversion_plan::native_collection_display_kind(&shape, facts).is_none()
                    || (!shape.native_allocation.shared() && thrown != "RuntimeException")
                {
                    return;
                }
                let Some(CallTargetPlan::NativeCollectionMethod { method, .. }) =
                    crate::call_plan::plan_native_collection_receiver_call_for_type(
                        receiver,
                        name,
                        Some(args),
                        &shape.java_ty,
                        env.type_env(),
                    )
                else {
                    return;
                };
                found = if thrown == "UnsupportedOperationException" {
                    method.mutates_receiver()
                } else if thrown == "RuntimeException" {
                    // Collection formatting invokes arbitrary element
                    // toString code and traverses a live fail-fast cursor.
                    method == NativeCollectionMethod::ToString
                } else if thrown == "NoSuchElementException" {
                    method.is_queue_endpoint() && !method.queue_endpoint_can_be_empty()
                } else if thrown == "NullPointerException" {
                    matches!(method, NativeCollectionMethod::QueueInsert { .. })
                } else {
                    matches!(
                        method,
                        NativeCollectionMethod::ListGet
                            | NativeCollectionMethod::ListSet
                            | NativeCollectionMethod::ListInsert
                            | NativeCollectionMethod::ListRemoveIndex
                    )
                };
            });
            found
        };
        let handles = |catches: &[CatchClause]| catches_intrinsic_exception(catches, thrown);
        if block_has_unhandled_expression_effect(
            body,
            env,
            &ExpressionEffect {
                expression: &expression,
                handles: &handles,
                learn: Some(&learn),
            },
        ) {
            return true;
        }
    }
    false
}

fn block_has_unhandled_shaped_call_null_check(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let switch_selectors = switch_selector_sites(body);
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let fallback_index;
    let external_index = match ctx.external_index() {
        Some(index) => index,
        None => {
            fallback_index = facts.external_lookup_index();
            &fallback_index
        }
    };
    let ctx = crate::expr_shape::ExprShapeContext::new(
        ctx.signatures(),
        ctx.representations(),
        Some(facts),
    )
    .with_external_index(external_index);
    let nullable = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        crate::expr_shape::infer_expr_shape_with_context(expr, env, ctx)
            .optional_element_type()
            .is_some()
    };
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |node| {
            if found {
                return;
            }
            if switch_selectors.contains(&(node as *const Expr)) && nullable(node, env) {
                found = true;
                return;
            }
            let (args, shapes) = match node {
                Expr::Call {
                    receiver,
                    name,
                    args,
                    resolved,
                    ..
                } => {
                    if receiver
                        .as_deref()
                        .is_some_and(|receiver| nullable(receiver, env))
                    {
                        let target =
                            crate::call_plan::plan_call_target_with_external_index_and_arity(
                                receiver.as_deref(),
                                name,
                                args.len(),
                                resolved.as_ref(),
                                facts,
                                external_index,
                                env.type_env(),
                            );
                        if target.receiver_expr().is_some() {
                            found = true;
                            return;
                        }
                    }
                    // No nullable value means no argument null check. Avoid
                    // constructing a complete argument plan for those calls.
                    if !args.iter().any(|arg| nullable(arg, env)) {
                        return;
                    }
                    let plan = crate::call_plan::plan_call_arguments_with_facts_and_args(
                        receiver.as_deref(),
                        name,
                        resolved.as_ref(),
                        args,
                        signatures,
                        facts,
                        env.type_env(),
                    );
                    let shapes = match plan {
                        crate::call_plan::ArgumentLoweringPlan::Shaped(shapes) => {
                            shapes.into_iter().map(Some).collect::<Vec<_>>()
                        }
                        crate::call_plan::ArgumentLoweringPlan::Slotted(slots) => slots
                            .into_iter()
                            .map(|slot| match slot {
                                crate::conversion_plan::Slot::Argument(shape)
                                | crate::conversion_plan::Slot::Assignment(shape) => Some(shape),
                                crate::conversion_plan::Slot::Index => {
                                    Some(crate::slot_shape::RustSlotShape::for_storage(
                                        JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
                                    ))
                                }
                                crate::conversion_plan::Slot::Condition => {
                                    Some(crate::slot_shape::RustSlotShape::for_storage(
                                        JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean),
                                    ))
                                }
                                _ => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                    (args, shapes)
                }
                Expr::New {
                    ty, args, resolved, ..
                } => {
                    if let [source] = args.as_slice()
                        && (nullable(source, env)
                            || matches!(source, Expr::Literal(crate::java_ir::Literal::Null)))
                        && matches!(
                            crate::call_plan::plan_native_collection_constructor_for_args(
                                ty,
                                args,
                                env.type_env(),
                                facts
                            ),
                            Some(
                                crate::call_plan::NativeCollectionNewTarget::FromSource { .. }
                                    | crate::call_plan::NativeCollectionNewTarget::FromSingleArg { .. }
                            )
                        )
                    {
                        found = true;
                        return;
                    }
                    let Some(call) = resolved else {
                        return;
                    };
                    (
                        args,
                        call.params
                            .iter()
                            .map(|param| {
                                matches!(param.ty, JavaType::Primitive(_)).then(|| {
                                    crate::slot_shape::RustSlotShape::for_storage(param.ty.clone())
                                })
                            })
                            .collect(),
                    )
                }
                Expr::ArrayAccess(receiver, _) => {
                    found = nullable(receiver, env);
                    return;
                }
                Expr::Cast {
                    ty: JavaType::Primitive(_),
                    expr,
                } => {
                    found = nullable(expr, env);
                    return;
                }
                _ => return,
            };
            found = args.iter().zip(shapes).any(|(arg, target)| {
                let Some(target) = target else {
                    return false;
                };
                let actual = crate::expr_shape::infer_expr_shape_with_context(arg, env, ctx);
                actual.optional_element_type().is_some()
                    && matches!(
                        crate::conversion_plan::optional_shape_coercion_policy_with_facts(
                            &actual, &target, facts,
                        ),
                        crate::conversion_plan::OptionalShapeCoercionPolicy::Unwrap { .. }
                    )
            });
        });
        found
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &catches_null_pointer_exception,
            learn: Some(&learn),
        },
    )
}

fn block_has_unhandled_checked_identity_null(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |node| {
            let Some(value) =
                crate::call_plan::checked_identity_source_argument(node, env.type_env(), facts)
            else {
                return;
            };
            let Expr::Call { resolved, .. } = node else {
                return;
            };
            let actual = crate::expr_shape::infer_expr_shape_with_context(value, env, ctx);
            found |= matches!(value, Expr::Literal(crate::java_ir::Literal::Null))
                || crate::call_plan::checked_identity_argument_has_null_carrier(
                    &actual,
                    resolved.as_ref(),
                );
        });
        found
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &catches_null_pointer_exception,
            learn: Some(&learn),
        },
    )
}

fn block_has_unhandled_checked_cast(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
) -> bool {
    let expression =
        |expr: &Expr, env: &crate::expr_shape::ShapeEnv| expr_has_checked_cast(expr, env, ctx);
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx))
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &catches_class_cast_exception,
            learn: Some(&learn),
        },
    )
}

fn nullable_receiver_throw_callables(
    program: &Program,
    signatures: Option<&crate::signature::SignaturePlan>,
    facts: &FactSet,
) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    let nullable_params = signatures
                        .and_then(|signatures| signatures.method_signature(ty, method))
                        .map(|signature| {
                            nullable_reference_binding_names_with_shapes(
                                &method.params,
                                signature.params().iter().map(|param| param.shape()),
                                signature.locals(),
                            )
                        })
                        .unwrap_or_else(|| nullable_reference_param_names(&method.params));
                    let callback_throws = signatures
                        .and_then(|signatures| {
                            signatures
                                .method_signature(ty, method)
                                .map(|signature| (signatures, signature))
                        })
                        .zip(
                            method
                                .body
                                .as_ref()
                                .filter(|body| block_has_callback_syntax(body)),
                        )
                        .is_some_and(|((signatures, signature), body)| {
                            let env = crate::expr_shape::shape_env_for_method(
                                ty, method, signature, signatures,
                            );
                            block_has_unhandled_nullable_callback_receiver(
                                body,
                                &env,
                                signature.locals(),
                                signatures,
                                facts,
                            )
                        });
                    if callback_throws
                        || (!nullable_params.is_empty()
                            && method.body.as_ref().is_some_and(|body| {
                                block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                            }))
                    {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    let nullable_params = signatures
                        .and_then(|signatures| signatures.constructor_signature(ty, constructor))
                        .map(|signature| {
                            nullable_reference_binding_names_with_shapes(
                                &constructor.params,
                                signature.params().iter().map(|param| param.shape()),
                                signature.locals(),
                            )
                        })
                        .unwrap_or_else(|| nullable_reference_param_names(&constructor.params));
                    let callback_throws = signatures
                        .and_then(|signatures| {
                            signatures
                                .constructor_signature(ty, constructor)
                                .map(|signature| (signatures, signature))
                        })
                        .zip(
                            constructor
                                .body
                                .as_ref()
                                .filter(|body| block_has_callback_syntax(body)),
                        )
                        .is_some_and(|((signatures, signature), body)| {
                            let env = crate::expr_shape::shape_env_for_constructor(
                                ty,
                                constructor,
                                signature,
                                signatures,
                            );
                            block_has_unhandled_nullable_callback_receiver(
                                body,
                                &env,
                                signature.locals(),
                                signatures,
                                facts,
                            )
                        });
                    if callback_throws
                        || (!nullable_params.is_empty()
                            && constructor.body.as_ref().is_some_and(|body| {
                                block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                            }))
                    {
                        keys.push(CallableKey::source_constructor_for_type(ty, constructor));
                    }
                }
                _ => {}
            }
        }
    }
    keys
}

fn block_has_callback_syntax(body: &Block) -> bool {
    let mut found = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        found |= matches!(expr, Expr::Lambda { .. } | Expr::MethodReference { .. });
    });
    found
}

fn block_has_unhandled_nullable_callback_receiver(
    body: &Block,
    env: &crate::expr_shape::ShapeEnv,
    locals: &[crate::signature::LocalPlan],
    signatures: &crate::signature::SignaturePlan,
    facts: &FactSet,
) -> bool {
    let ctx = crate::expr_shape::ExprShapeContext::new(Some(signatures), None, Some(facts));
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
            if found {
                return;
            }
            if let Expr::MethodReference { target, .. } = expr {
                if let Some(bound) =
                    crate::method_reference_plan::bound_method_reference_receiver(target, env)
                {
                    found = crate::expr_shape::infer_expr_shape_with_context(
                        &bound.source_expr(),
                        env,
                        ctx,
                    )
                    .optional_element_type()
                    .is_some();
                }
            }
            let Some(stream) =
                crate::stream::build_stream_plan_with_shape_context(expr, env, ctx, facts)
            else {
                return;
            };
            stream.visit_consumed_callbacks(&mut |lambda, input| {
                if input.optional_element_type().is_none() {
                    return;
                }
                let names = HashSet::from([
                    lambda.param_name().to_string(),
                    lambda.rust_param_name().to_string(),
                ]);
                found |= match lambda.body() {
                    crate::stream::StreamLambdaBody::Expr(expr) => {
                        expr_has_nullable_receiver_deref(expr, &names)
                    }
                    crate::stream::StreamLambdaBody::Block(block) => {
                        block_has_unhandled_nullable_receiver_deref(block, &names)
                    }
                    crate::stream::StreamLambdaBody::MethodReference { target, method } => {
                        crate::method_reference_shape::single_input_reference_has_nullable_receiver(
                            target, method, input, env, signatures, facts,
                        )
                    }
                };
            });
        });
        found
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(stmt, env, locals, signatures, Some(ctx));
    };
    block_has_unhandled_expression_effect(
        body,
        env,
        &ExpressionEffect {
            expression: &expression,
            handles: &catches_null_pointer_exception,
            learn: Some(&learn),
        },
    )
}

fn direct_throw_callables(program: &Program, facts: &FactSet) -> Vec<CallableKey> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    let mut keys = Vec::new();
    for ty in types {
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    if method
                        .body
                        .as_ref()
                        .is_some_and(|body| block_has_unhandled_direct_throw(body, facts, &[]))
                    {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    if constructor
                        .body
                        .as_ref()
                        .is_some_and(|body| block_has_unhandled_direct_throw(body, facts, &[]))
                    {
                        keys.push(CallableKey::source_constructor_for_type(ty, constructor));
                    }
                }
                _ => {}
            }
        }
    }
    keys
}

fn block_has_unhandled_direct_throw(block: &Block, facts: &FactSet, handled: &[JavaType]) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_has_unhandled_direct_throw(stmt, facts, handled))
}

fn stmt_has_unhandled_direct_throw(stmt: &Stmt, facts: &FactSet, handled: &[JavaType]) -> bool {
    match stmt {
        Stmt::Throw(expr) => !catch_types_handle_throw_expr(handled, expr, facts),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_has_unhandled_direct_throw(block, facts, handled)
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_has_unhandled_direct_throw(expr, facts, handled)),
        Stmt::Assign { target, value } => {
            expr_has_unhandled_direct_throw(target, facts, handled)
                || expr_has_unhandled_direct_throw(value, facts, handled)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
            expr_has_unhandled_direct_throw(expr, facts, handled)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_unhandled_direct_throw(condition, facts, handled)
                || block_has_unhandled_direct_throw(then_block, facts, handled)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_direct_throw(block, facts, handled))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_has_unhandled_direct_throw(condition, facts, handled)
                || block_has_unhandled_direct_throw(body, facts, handled)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_has_unhandled_direct_throw(stmt, facts, handled))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_has_unhandled_direct_throw(expr, facts, handled))
                || update
                    .iter()
                    .any(|expr| expr_has_unhandled_direct_throw(expr, facts, handled))
                || block_has_unhandled_direct_throw(body, facts, handled)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_unhandled_direct_throw(iterable, facts, handled)
                || block_has_unhandled_direct_throw(body, facts, handled)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_unhandled_direct_throw(selector, facts, handled)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(|label| switch_label_has_unhandled_direct_throw(label, facts, handled))
                        || case
                            .statements
                            .iter()
                            .any(|stmt| stmt_has_unhandled_direct_throw(stmt, facts, handled))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut guarded_handled = handled.to_vec();
            for catch in catches {
                guarded_handled.extend(catch.types.iter().cloned());
            }
            resources
                .iter()
                .any(|stmt| stmt_has_unhandled_direct_throw(stmt, facts, &guarded_handled))
                || block_has_unhandled_direct_throw(body, facts, &guarded_handled)
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_direct_throw(&catch.body, facts, handled))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_direct_throw(block, facts, handled))
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_has_unhandled_direct_throw(expr, facts, handled)),
        Stmt::LocalType(_) => false,
    }
}

fn expr_has_unhandled_direct_throw(expr: &Expr, facts: &FactSet, handled: &[JavaType]) -> bool {
    match expr {
        Expr::Lambda { .. } => false,
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_unhandled_direct_throw(condition, facts, handled)
                || expr_has_unhandled_direct_throw(then_expr, facts, handled)
                || expr_has_unhandled_direct_throw(else_expr, facts, handled)
        }
        Expr::Switch { selector, cases } => {
            expr_has_unhandled_direct_throw(selector, facts, handled)
                || cases.iter().any(|case| {
                    expr_has_unhandled_direct_throw(&case.value, facts, handled)
                        || case.labels.iter().any(|label| {
                            switch_label_has_unhandled_direct_throw(label, facts, handled)
                        })
                })
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_has_unhandled_direct_throw(receiver, facts, handled)
        }
        Expr::Binary { left, right, .. } => {
            expr_has_unhandled_direct_throw(left, facts, handled)
                || expr_has_unhandled_direct_throw(right, facts, handled)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_has_unhandled_direct_throw(receiver, facts, handled))
                || args
                    .iter()
                    .any(|arg| expr_has_unhandled_direct_throw(arg, facts, handled))
        }
        Expr::New { args, .. } => args
            .iter()
            .any(|arg| expr_has_unhandled_direct_throw(arg, facts, handled)),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_has_unhandled_direct_throw(expr, facts, handled))
                || initializer.as_ref().is_some_and(|items| {
                    items
                        .iter()
                        .any(|expr| expr_has_unhandled_direct_throw(expr, facts, handled))
                })
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .any(|expr| expr_has_unhandled_direct_throw(expr, facts, handled)),
        Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_)
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super => false,
    }
}

fn switch_label_has_unhandled_direct_throw(
    label: &crate::java_ir::SwitchLabel,
    facts: &FactSet,
    handled: &[JavaType],
) -> bool {
    match label {
        crate::java_ir::SwitchLabel::Default => false,
        crate::java_ir::SwitchLabel::Expr(expr) => {
            expr_has_unhandled_direct_throw(expr, facts, handled)
        }
    }
}

fn catch_types_handle_throw_expr(catch_types: &[JavaType], expr: &Expr, facts: &FactSet) -> bool {
    let Some(thrown_name) = thrown_expr_type_name(expr) else {
        return catch_types.iter().any(|catch_ty| {
            matches!(
                exception_class_name(catch_ty).as_str(),
                "Throwable" | "Exception" | "RuntimeException"
            )
        });
    };
    catch_types.iter().any(|catch_ty| {
        let catch_name = exception_class_name(catch_ty);
        thrown_name == catch_name
            || facts
                .hierarchy_subtype_policy(
                    &JavaType::Named {
                        name: thrown_name.clone(),
                        args: Vec::new(),
                    },
                    catch_ty,
                )
                .reaches()
            || crate::stdlib::is_known_java_subtype(&thrown_name, &catch_name)
    })
}

fn thrown_expr_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::New { ty, .. } | Expr::Cast { ty, .. } => Some(exception_class_name(ty)),
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

fn nullable_reference_param_names(params: &[crate::java_ir::Param]) -> HashSet<String> {
    let mut names = HashSet::new();
    for param in params {
        if java_type_can_be_null(&param.ty) {
            names.insert(param.name.clone());
            names.insert(param.source_name.clone());
        }
    }
    names
}

fn nullable_reference_binding_names_with_shapes<'a>(
    params: &[crate::java_ir::Param],
    shapes: impl IntoIterator<Item = &'a crate::slot_shape::RustSlotShape>,
    locals: &[crate::signature::LocalPlan],
) -> HashSet<String> {
    let mut names = HashSet::new();
    for (param, shape) in params.iter().zip(shapes) {
        if java_type_can_be_null(&param.ty) && shape.optional_element_type().is_some() {
            names.insert(param.name.clone());
            names.insert(param.source_name.clone());
        }
    }
    // A solved nullable local dereference throws just like a nullable formal.
    // Use the existing signature proof rather than guessing from an initializer
    // spelling, so conditionals, aliases and source-return contracts agree.
    names.extend(
        locals
            .iter()
            .filter(|local| {
                local.shape().optional_element_type().is_some() && !local.proven_non_null()
            })
            .map(|local| local.name().to_string()),
    );
    names
}

fn java_type_can_be_null(ty: &JavaType) -> bool {
    !matches!(ty, JavaType::Void | JavaType::Primitive(_))
}

fn catches_null_pointer_exception(catches: &[CatchClause]) -> bool {
    catches_intrinsic_exception(catches, "NullPointerException")
}

fn block_has_unhandled_nullable_receiver_deref(block: &Block, names: &HashSet<String>) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_has_unhandled_nullable_receiver_deref(stmt, names))
}

fn stmt_has_unhandled_nullable_receiver_deref(stmt: &Stmt, names: &HashSet<String>) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let guarded_counts = !catches_null_pointer_exception(catches);
            (guarded_counts
                && (resources
                    .iter()
                    .any(|stmt| stmt_has_unhandled_nullable_receiver_deref(stmt, names))
                    || block_has_unhandled_nullable_receiver_deref(body, names)))
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_nullable_receiver_deref(&catch.body, names))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_nullable_receiver_deref(block, names))
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_has_nullable_receiver_deref(expr, names)),
        Stmt::Assign { target, value } => {
            expr_has_nullable_receiver_deref(target, names)
                || expr_has_nullable_receiver_deref(value, names)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_has_nullable_receiver_deref(expr, names)
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_nullable_receiver_deref(condition, names)
                || block_has_unhandled_nullable_receiver_deref(then_block, names)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_nullable_receiver_deref(block, names))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_has_nullable_receiver_deref(condition, names)
                || block_has_unhandled_nullable_receiver_deref(body, names)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_has_unhandled_nullable_receiver_deref(stmt, names))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_has_nullable_receiver_deref(expr, names))
                || update
                    .iter()
                    .any(|expr| expr_has_nullable_receiver_deref(expr, names))
                || block_has_unhandled_nullable_receiver_deref(body, names)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_is_nullable_param_ref(iterable, names)
                || expr_has_nullable_receiver_deref(iterable, names)
                || block_has_unhandled_nullable_receiver_deref(body, names)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_nullable_receiver_deref(selector, names)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(|label| switch_label_has_nullable_receiver_deref(label, names))
                        || case
                            .statements
                            .iter()
                            .any(|stmt| stmt_has_unhandled_nullable_receiver_deref(stmt, names))
                })
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_nullable_receiver_deref(body, names)
        }
        Stmt::LocalType(nested) => type_has_nullable_receiver_throw(nested),
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_has_nullable_receiver_deref(expr, names)),
    }
}

fn type_has_nullable_receiver_throw(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => {
            let nullable_params = nullable_reference_param_names(&method.params);
            !nullable_params.is_empty()
                && method.body.as_ref().is_some_and(|body| {
                    block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                })
        }
        Member::Constructor(constructor) => {
            let nullable_params = nullable_reference_param_names(&constructor.params);
            !nullable_params.is_empty()
                && constructor.body.as_ref().is_some_and(|body| {
                    block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                })
        }
        Member::Type(nested) => type_has_nullable_receiver_throw(nested),
        _ => false,
    })
}

fn switch_label_has_nullable_receiver_deref(
    label: &crate::java_ir::SwitchLabel,
    names: &HashSet<String>,
) -> bool {
    match label {
        crate::java_ir::SwitchLabel::Expr(expr) => expr_has_nullable_receiver_deref(expr, names),
        crate::java_ir::SwitchLabel::Default => false,
    }
}

fn expr_has_nullable_receiver_deref(expr: &Expr, names: &HashSet<String>) -> bool {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            expr_is_nullable_param_ref(receiver, names)
                || expr_has_nullable_receiver_deref(receiver, names)
        }
        Expr::ArrayAccess(receiver, index) => {
            expr_is_nullable_param_ref(receiver, names)
                || expr_has_nullable_receiver_deref(receiver, names)
                || expr_has_nullable_receiver_deref(index, names)
        }
        Expr::Call { receiver, args, .. } => {
            receiver.as_deref().is_some_and(|receiver| {
                expr_is_nullable_param_ref(receiver, names)
                    || expr_has_nullable_receiver_deref(receiver, names)
            }) || args
                .iter()
                .any(|arg| expr_has_nullable_receiver_deref(arg, names))
        }
        Expr::Binary { left, right, .. } => {
            expr_has_nullable_receiver_deref(left, names)
                || expr_has_nullable_receiver_deref(right, names)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_has_nullable_receiver_deref(expr, names)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_nullable_receiver_deref(condition, names)
                || expr_has_nullable_receiver_deref(then_expr, names)
                || expr_has_nullable_receiver_deref(else_expr, names)
        }
        Expr::Switch { selector, cases } => {
            expr_has_nullable_receiver_deref(selector, names)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(|label| switch_label_has_nullable_receiver_deref(label, names))
                        || expr_has_nullable_receiver_deref(&case.value, names)
                })
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_has_nullable_receiver_deref(qualifier, names))
                || args
                    .iter()
                    .any(|arg| expr_has_nullable_receiver_deref(arg, names))
                || body
                    .iter()
                    .flatten()
                    .any(|member| member_has_nullable_receiver_deref(member, names))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_has_nullable_receiver_deref(expr, names))
                || initializer
                    .iter()
                    .flatten()
                    .any(|expr| expr_has_nullable_receiver_deref(expr, names))
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_has_nullable_receiver_deref(expr, names)),
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_has_nullable_receiver_deref(expr, names),
            crate::java_ir::LambdaBody::Block(block) => {
                block_has_unhandled_nullable_receiver_deref(block, names)
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

fn member_has_nullable_receiver_deref(member: &Member, names: &HashSet<String>) -> bool {
    match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_has_nullable_receiver_deref(expr, names)),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|body| block_has_unhandled_nullable_receiver_deref(body, names)),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(|body| block_has_unhandled_nullable_receiver_deref(body, names)),
        Member::Type(nested) => type_has_nullable_receiver_throw(nested),
        Member::Unsupported { .. } => false,
    }
}

fn expr_is_nullable_param_ref(expr: &Expr, names: &HashSet<String>) -> bool {
    match expr {
        Expr::Var(name) => names.contains(name),
        Expr::Cast { expr, .. } => expr_is_nullable_param_ref(expr, names),
        _ => false,
    }
}

/// Groups each declared method with the supertype method it overrides or
/// implements, so fallibility can be equalized across the group.
pub(crate) fn override_groups_for_program(program: &Program) -> Vec<Vec<CallableKey>> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls(ty, &mut types);
    }
    // Match the same source identities as inheritance/member lookup, including
    // imported nested interfaces. Cache each owner-scoped hierarchy edge for
    // this immutable program instead of rescanning it for every method slot.
    let mut owner_lookup = HashMap::new();
    let mut groups: HashMap<(String, String), Vec<CallableKey>> = HashMap::new();
    for ty in &types {
        for member in &ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            if method.modifiers.contains(&Modifier::Static) {
                continue;
            }
            // Key on the *Rust* name, as every other `CallableKey::Method` in
            // the program does. This used to use `method.name` — the Java name
            // — so a group's keys matched the solved plan's keys only when the
            // two spellings happened to coincide, which for anything
            // camelCased or overloaded they do not. The equalization therefore
            // did nothing for most methods, and `trait_method_exception_plan`
            // carried a second `|=` pass to compensate. Same defect as
            // [0009](../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md),
            // one layer down: identity spelled two ways.
            for (supertype, super_method) in
                supertype_methods_matching_slot(ty, method, program, &mut owner_lookup)
            {
                let rust_name = source_method_rust_name(super_method);
                let entry = groups
                    .entry((supertype.clone(), rust_name.clone()))
                    .or_default();
                let declaring = CallableKey::source_method(supertype, super_method);
                if !entry.contains(&declaring) {
                    entry.push(declaring);
                }
                let implementing = CallableKey::source_method_for_type(ty, method);
                if !entry.contains(&implementing) {
                    entry.push(implementing);
                }
            }
        }
    }
    groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// Transitive supertype methods matching `method` by erased Java signature.
fn supertype_methods_matching_slot<'a>(
    ty: &TypeDecl,
    method: &MethodDecl,
    program: &'a Program,
    owner_lookup: &mut HashMap<(String, String), Option<&'a TypeDecl>>,
) -> Vec<(String, &'a MethodDecl)> {
    let mut matches = Vec::new();
    let wanted = JavaCallableSlotKey::source_method(method);
    let mut pending = ty
        .extends
        .iter()
        .chain(ty.implements.iter())
        .map(|name| (ty.name.clone(), crate::parser::parse_hierarchy_type(name)))
        .collect::<Vec<_>>();
    let mut seen = std::collections::HashSet::new();
    while let Some((owner_name, super_ref)) = pending.pop() {
        let JavaType::Named { name, args } = super_ref else {
            continue;
        };
        let Some(supertype) = *owner_lookup
            .entry((owner_name.clone(), name.clone()))
            .or_insert_with(|| {
                crate::inherited_members::find_type_decl_for_owner_context(
                    program,
                    &owner_name,
                    &name,
                )
            })
        else {
            continue;
        };
        if !seen.insert(supertype.name.clone()) {
            continue;
        }
        let substitutions = supertype
            .type_params
            .iter()
            .cloned()
            .zip(args)
            .collect::<HashMap<_, _>>();
        for member in &supertype.members {
            let Member::Method(candidate) = member else {
                continue;
            };
            // Java override identity is checked after substituting the
            // implemented supertype's arguments.  Comparing `write(T)`
            // directly with `write(String)` loses generic interface override
            // groups and prevents fallibility from reaching the trait.
            let mut specialized = candidate.clone();
            for param in &mut specialized.params {
                param.ty =
                    crate::resolve::substitute_method_type_variables(&param.ty, &substitutions);
            }
            if JavaCallableSlotKey::source_method(&specialized) == wanted {
                matches.push((supertype.name.clone(), candidate));
            }
        }
        pending.extend(
            supertype
                .extends
                .iter()
                .chain(supertype.implements.iter())
                .map(|parent| crate::parser::parse_hierarchy_type(parent))
                .map(|parent| {
                    (
                        supertype.name.clone(),
                        crate::resolve::substitute_method_type_variables(&parent, &substitutions),
                    )
                }),
        );
    }
    matches
}

fn collect_type_decls<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>) {
    out.push(ty);
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decls(nested, out);
        }
    }
}

fn seed_callable_fallibility(key: &CallableKey, facts: &FactSet) -> CallableFallibility {
    let reason = key
        .method_identity_parts()
        .or_else(|| key.constructor_identity_parts())
        .and_then(|(type_name, method_name)| {
            facts
                .method_effect_for(type_name, method_name)
                .is_some_and(|fact| !fact.thrown.is_empty())
                .then_some(FallibilityReason::MethodThrowFact)
        })
        .or_else(|| {
            key.external_method_parts()
                .and_then(|(receiver_type, method_name, arg_count)| {
                    facts
                        .external_api_effect_for_receiver_with_arity_or_rust_name(
                            receiver_type,
                            method_name,
                            Some(arg_count),
                        )
                        .is_some_and(|fact| !fact.thrown.is_empty())
                        .then_some(FallibilityReason::ExternalThrowFact)
                })
        })
        .or_else(|| {
            if let CallableKey::ExternalConstructor {
                type_name,
                arg_count,
            } = key
            {
                return facts
                    .external_constructor_rust_result_for(type_name, *arg_count)
                    .is_some_and(|fact| fact.returns_result)
                    .then_some(FallibilityReason::ExternalThrowFact);
            }
            let (receiver, method, arity) = key.external_method_parts()?;
            facts
                .external_rust_result_for_receiver(receiver, method, Some(arity))
                .is_some_and(|fact| fact.returns_result)
                .then_some(FallibilityReason::ExternalThrowFact)
        })
        .or_else(|| {
            let (Some(owner), method, arity) = key.external_method_parts()? else {
                return None;
            };
            (owner == "Iterator"
                && arity == 0
                && crate::stdlib::IteratorProtocolOperation::ALL
                    .iter()
                    .any(|operation| operation.checked_method() == method)
                && !crate::call_plan::java_type_owner_is_parsed_source(
                    owner,
                    &TypeEnv::new(),
                    facts,
                ))
            .then_some(FallibilityReason::IteratorProtocolThrow)
        })
        .or_else(|| {
            let (Some(owner), method, arity) = key.external_method_parts()? else {
                return None;
            };
            let sam = crate::stdlib::functional_interface_shape(
                &crate::stdlib::canonical_external_receiver_type_key(owner),
            )?;
            if crate::call_plan::java_type_owner_is_parsed_source(owner, &TypeEnv::new(), facts) {
                return None;
            }
            // This runtime entry is distinct from the Java interface method,
            // whose source implementations/plain adapters have their own ABI.
            (sam.inputs().len() == arity && sam.result_method_name() == method)
                .then_some(FallibilityReason::StoredCallbackThrow)
        });
    let mut fallibility = CallableFallibility::default();
    if let Some(reason) = reason {
        fallibility.returns_result = true;
        fallibility.reasons.push(reason);
    }
    fallibility
}

pub fn plan_try_stmt<'a>(
    resources: &'a [Stmt],
    body: &'a Block,
    catches: &'a [CatchClause],
    finally_block: Option<&'a Block>,
) -> TryStmtPlan<'a> {
    if let Some(finally_block) = finally_block {
        let completion = plan_block_exception_completion(body);
        let finally_completion = plan_block_exception_completion(finally_block);
        if resources.is_empty() {
            if catches.is_empty() && finally_completion_allows_try_finally(finally_completion) {
                return TryStmtPlan::FinallyOnly {
                    body,
                    finally_block,
                    completion,
                };
            }
            if finally_completion != TryBlockCompletion::FallsThroughUnit {
                if finally_completion == TryBlockCompletion::Terminates
                    && completion == TryBlockCompletion::Terminates
                    && catches
                        .iter()
                        .all(|catch| block_terminates_with_exception_boundary(&catch.body))
                {
                    return TryStmtPlan::CatchFinally {
                        body,
                        catches,
                        finally_block,
                        completion,
                    };
                }
                return TryStmtPlan::Unsupported;
            }
            return TryStmtPlan::CatchFinally {
                body,
                catches,
                finally_block,
                completion,
            };
        }
        if catches.is_empty()
            && finally_completion == TryBlockCompletion::FallsThroughUnit
            && try_resources_are_declarations(resources)
            && !block_has_abrupt_loop_completion(body)
            && !block_has_abrupt_loop_completion(finally_block)
        {
            return TryStmtPlan::WithResourcesFinally {
                resources,
                body,
                finally_block,
                completion,
            };
        }
        if finally_completion == TryBlockCompletion::FallsThroughUnit
            && try_resources_are_declarations(resources)
            && !block_has_abrupt_loop_completion(body)
            && catches
                .iter()
                .all(|catch| !block_has_abrupt_loop_completion(&catch.body))
            && !block_has_abrupt_loop_completion(finally_block)
        {
            if completion == TryBlockCompletion::Terminates
                && catches
                    .iter()
                    .any(|catch| !block_terminates_with_exception_boundary(&catch.body))
            {
                return TryStmtPlan::Unsupported;
            }
            return TryStmtPlan::WithResourcesCatchFinally {
                resources,
                body,
                catches,
                finally_block,
                completion,
            };
        }
        return TryStmtPlan::Unsupported;
    }
    let completion = plan_block_exception_completion(body);
    if !resources.is_empty() {
        if !try_resources_are_declarations(resources) {
            return TryStmtPlan::Unsupported;
        }
        if catches
            .iter()
            .any(|catch| !block_terminates_with_exception_boundary(&catch.body))
        {
            return TryStmtPlan::Unsupported;
        }
        return TryStmtPlan::WithResources {
            resources,
            body,
            catches,
            completion,
        };
    }
    if catches.is_empty() {
        return TryStmtPlan::Unsupported;
    }
    TryStmtPlan::CatchOnly {
        body,
        catches,
        completion,
    }
}

fn block_has_abrupt_loop_completion(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_abrupt_loop_completion)
}

fn stmt_has_abrupt_loop_completion(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_abrupt_loop_completion(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_abrupt_loop_completion)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_has_abrupt_loop_completion(body),
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_has_abrupt_loop_completion)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_has_abrupt_loop_completion)
                || block_has_abrupt_loop_completion(body)
                || catches
                    .iter()
                    .any(|catch| block_has_abrupt_loop_completion(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_abrupt_loop_completion)
        }
        Stmt::LocalType(ty) => ty.members.iter().any(|member| match member {
            Member::Method(method) => method
                .body
                .as_ref()
                .is_some_and(block_has_abrupt_loop_completion),
            Member::Constructor(ctor) => ctor
                .body
                .as_ref()
                .is_some_and(block_has_abrupt_loop_completion),
            Member::Type(nested) => nested.members.iter().any(|member| match member {
                Member::Method(method) => method
                    .body
                    .as_ref()
                    .is_some_and(block_has_abrupt_loop_completion),
                Member::Constructor(ctor) => ctor
                    .body
                    .as_ref()
                    .is_some_and(block_has_abrupt_loop_completion),
                _ => false,
            }),
            _ => false,
        }),
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Unsupported(_) => false,
    }
}

fn finally_completion_allows_try_finally(completion: TryBlockCompletion) -> bool {
    matches!(
        completion,
        TryBlockCompletion::FallsThroughUnit | TryBlockCompletion::Terminates
    )
}

pub(crate) fn try_resource_names(resources: &[Stmt]) -> Vec<String> {
    resources
        .iter()
        .filter_map(|resource| match resource {
            Stmt::VarDecl { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn try_resources_are_declarations(resources: &[Stmt]) -> bool {
    resources.iter().all(stmt_is_resource_declaration)
}

fn stmt_is_resource_declaration(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::VarDecl {
            initializer: Some(_),
            ..
        }
    )
}

pub(crate) fn catch_match_class_names(
    current_type: Option<&str>,
    current_method: Option<&str>,
    clause_index: usize,
    catch: &CatchClause,
    facts: &FactSet,
) -> Vec<String> {
    let mut names = catch_assignable_class_names(&catch.types, facts);
    let fact_names = current_type
        .zip(current_method)
        .and_then(|(type_name, method_name)| {
            facts
                .catch_facts_for(type_name, method_name)
                .into_iter()
                .find(|fact| fact.clause_index == clause_index && fact.types == catch.types)
                .map(|fact| fact.assignable_from.as_slice())
        });
    if let Some(fact_names) = fact_names {
        for ty in fact_names {
            let name = exception_class_name(ty);
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn catch_assignable_class_names(catch_types: &[JavaType], facts: &FactSet) -> Vec<String> {
    let mut out = catch_types
        .iter()
        .map(exception_class_name)
        .collect::<Vec<_>>();
    for exception in &facts.exception_facts {
        let exception_ty = JavaType::Named {
            name: exception.type_name.clone(),
            args: Vec::new(),
        };
        if catch_types.iter().any(|catch_ty| {
            exception_class_name(catch_ty) == exception.type_name
                || facts
                    .hierarchy_subtype_policy(&exception_ty, catch_ty)
                    .reaches()
                || crate::stdlib::is_known_java_subtype(
                    &exception.type_name,
                    &exception_class_name(catch_ty),
                )
        }) && !out.contains(&exception.type_name)
        {
            out.push(exception.type_name.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn block_terminates_with_exception_boundary(block: &Block) -> bool {
    block
        .statements
        .last()
        .is_some_and(stmt_terminates_with_exception_boundary)
}

fn stmt_terminates_with_exception_boundary(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Throw(_) => true,
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_terminates_with_exception_boundary(block)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_terminates_with_exception_boundary(then_block)
                && else_block
                    .as_ref()
                    .is_some_and(block_terminates_with_exception_boundary)
        }
        Stmt::Switch { cases, .. } => switch_all_paths_reach_exception_boundary(cases),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => matches!(
            plan_try_stmt(resources, body, catches, finally_block.as_ref()),
            TryStmtPlan::CatchOnly {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::WithResources {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::WithResourcesFinally {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::WithResourcesCatchFinally {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::FinallyOnly {
                completion: TryBlockCompletion::Terminates,
                ..
            } | TryStmtPlan::CatchFinally {
                completion: TryBlockCompletion::Terminates,
                ..
            }
        ),
        _ => false,
    }
}

pub(crate) fn plan_block_exception_completion(block: &Block) -> TryBlockCompletion {
    if block_terminates_with_exception_boundary(block) {
        return TryBlockCompletion::Terminates;
    }
    TryBlockCompletion::FallsThroughUnit
}

fn switch_all_paths_reach_exception_boundary(cases: &[SwitchCase]) -> bool {
    if cases.is_empty()
        || !cases.iter().any(|case| {
            case.labels
                .iter()
                .any(|label| matches!(label, SwitchLabel::Default))
        })
    {
        return false;
    }
    (0..cases.len()).all(|start| switch_path_reaches_exception_boundary(cases, start))
}

fn switch_path_reaches_exception_boundary(cases: &[SwitchCase], start: usize) -> bool {
    cases[start..]
        .iter()
        .flat_map(|case| &case.statements)
        .find_map(|stmt| match stmt {
            Stmt::Return(_) | Stmt::Throw(_) => Some(true),
            Stmt::Break(_) => Some(false),
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. }
                if block_terminates_with_exception_boundary(block) =>
            {
                Some(true)
            }
            Stmt::If {
                then_block,
                else_block: Some(else_block),
                ..
            } if block_terminates_with_exception_boundary(then_block)
                && block_terminates_with_exception_boundary(else_block) =>
            {
                Some(true)
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } if matches!(
                plan_try_stmt(resources, body, catches, finally_block.as_ref()),
                TryStmtPlan::CatchOnly {
                    completion: TryBlockCompletion::Terminates,
                    ..
                } | TryStmtPlan::WithResources {
                    completion: TryBlockCompletion::Terminates,
                    ..
                } | TryStmtPlan::WithResourcesFinally {
                    completion: TryBlockCompletion::Terminates,
                    ..
                } | TryStmtPlan::WithResourcesCatchFinally {
                    completion: TryBlockCompletion::Terminates,
                    ..
                } | TryStmtPlan::FinallyOnly {
                    completion: TryBlockCompletion::Terminates,
                    ..
                } | TryStmtPlan::CatchFinally {
                    completion: TryBlockCompletion::Terminates,
                    ..
                }
            ) =>
            {
                Some(true)
            }
            _ => None,
        })
        .unwrap_or(false)
}

fn collect_method_exception_plans_for_type(
    ty: &TypeDecl,
    facts: &FactSet,
    fallibility: &ProgramFallibilityPlan,
    methods: &mut BTreeMap<(String, String), MethodExceptionPlan>,
) {
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                // One store. The solved plan already carries the declared
                // `throws` facts that the deleted second producer re-derived.
                let plan = MethodExceptionPlan::new(
                    fallibility.returns_result(&CallableKey::source_method_for_type(ty, method)),
                );
                methods.insert(method_exception_key(ty, method), plan);
            }
            Member::Type(nested) => {
                collect_method_exception_plans_for_type(nested, facts, fallibility, methods)
            }
            _ => {}
        }
    }
}

fn method_exception_key(ty: &TypeDecl, method: &MethodDecl) -> (String, String) {
    (ty.name.clone(), source_method_rust_name(method))
}

pub(crate) fn plan_throw_expr(expr: &Expr) -> ThrowExprPlan<'_> {
    match expr {
        Expr::New { ty, args, .. } => ThrowExprPlan::NewException {
            class_name: exception_class_name(ty),
            args,
        },
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } if cause_or_self_rethrow_target(condition, then_expr, else_expr).is_some() => {
            let (wrapper, class_name) =
                cause_or_self_rethrow_target(condition, then_expr, else_expr)
                    .expect("checked cause/self rethrow pattern");
            ThrowExprPlan::CauseOrSelfIfInstance {
                wrapper,
                class_names: vec![class_name],
            }
        }
        Expr::Var(_) => ThrowExprPlan::ExistingException(expr),
        _ => ThrowExprPlan::FallbackValue(expr),
    }
}

fn cause_or_self_rethrow_target<'a>(
    condition: &'a Expr,
    then_expr: &'a Expr,
    else_expr: &'a Expr,
) -> Option<(&'a Expr, String)> {
    let Expr::InstanceOf {
        expr: cause, ty, ..
    } = condition
    else {
        return None;
    };
    let Expr::Cast { ty: cast_ty, expr } = then_expr else {
        return None;
    };
    if exception_class_name(ty) != exception_class_name(cast_ty) || expr.as_ref() != cause.as_ref()
    {
        return None;
    }
    let wrapper = exception_cause_receiver(cause)?;
    (else_expr == wrapper).then(|| (wrapper, exception_class_name(ty)))
}

fn exception_cause_receiver(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    (args.is_empty() && name == "getCause").then_some(receiver.as_ref())
}

pub(crate) fn exception_class_name(class_ty: &JavaType) -> String {
    match class_ty {
        JavaType::Named { name, .. } => name.clone(),
        JavaType::Unknown(raw) => raw.clone(),
        _ => "JavaException".to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn anonymous_creation_keeps_initializer_effects_but_not_deferred_division() {
        let parsed = parse_java_source(
            r#"
interface Op { int apply(int value); }
class Factory {
    static Op lazy() { return new Op() { public int apply(int value) { return 42 / value; } }; }
    static Op eager(int divisor) { return new Op() {
        int initial = 42 / divisor;
        public int apply(int value) { return initial; }
    }; }
    static Op floating(double divisor) { return new Op() {
        double initial = 1.0 / divisor;
        public int apply(int value) { return (int) initial; }
    }; }
    static Op lambda() { return value -> 42 / value; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        for (name, expected) in [
            ("lazy", false),
            ("eager", true),
            ("floating", false),
            ("lambda", false),
        ] {
            let key = crate::usage::CallableKey::method_identity("Factory", name);
            assert_eq!(
                plan.returns_result(&key),
                expected,
                "{name}: {:?}",
                plan.callable(&key)
            );
        }
    }

    #[test]
    fn mapped_result_abis_seed_foreign_method_and_constructor_fallibility() {
        let parsed = crate::external_packs::parse_external_pack_manifest(
            "external-pack dep.value dep.Value rust-crate-mapping mapped target=value_runtime\nexternal-pack-constructor dep.value dep.Value 1 mapped rust-type=Value rust=new rust-result=true\nexternal-pack-api dep.value dep.Value read mapped getter read-only definitely-non-null args=0 return=int rust=read rust-result=true\n",
        );
        assert!(!parsed.diagnostics().has_errors());
        for key in [
            crate::usage::CallableKey::external_constructor("dep.Value", 1),
            crate::usage::CallableKey::external_method(Some("dep.Value"), "read", 0),
        ] {
            assert!(
                super::seed_callable_fallibility(&key, parsed.facts()).returns_result(),
                "{key:?}"
            );
        }
        assert!(
            !super::seed_callable_fallibility(
                &crate::usage::CallableKey::external_constructor("dep.Value", 2),
                parsed.facts(),
            )
            .returns_result()
        );
    }

    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::{Literal, Modifier, PrimitiveType, TypeKind};
    use crate::parser::parse_java_source;
    use crate::resolve::resolve_program;
    use crate::semantic_analysis::analyze_program;

    #[test]
    fn native_array_copy_effects_use_jdk_handlers_and_preserve_source_precedence() {
        let parsed = parse_java_source(
            r#"
import java.util.Arrays;
class CopyEffects {
    static byte[] copy(byte[] input, int size) { return Arrays.copyOf(input, size); }
    static byte[] range(byte[] input, int from, int to) { return Arrays.copyOfRange(input, from, to); }
    static byte[] caught(byte[] input) {
        try { return Arrays.copyOfRange(input, -1, 3); }
        catch (RuntimeException error) { return input; }
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let key = CallableKey::source_method_for_type(owner, method);
            assert_eq!(
                plan.returns_result(&key),
                method.name != "caught",
                "{}: {:?}",
                method.name,
                plan.callable(&key)
            );
        }
        let parsed = parse_java_source(
            r#"
class Arrays { static byte[] copyOf(byte[] input, int size) { return input; } }
class CopySource { static byte[] run(byte[] input) { return Arrays.copyOf(input, 1); } }
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let owner = &resolved.program.types[1];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let key = CallableKey::source_method_for_type(owner, method);
        assert!(
            !plan
                .callable(&key)
                .unwrap()
                .reasons()
                .contains(&FallibilityReason::NativeArrayCopyThrow)
        );
    }

    #[test]
    fn inherited_generic_hash_effect_uses_the_childs_field_frame() {
        let parsed = parse_java_source(
            r#"
            class Base<T> { T value; public int hashCode() { return 0; } }
            class Child<U> extends Base<U> {
                public int hashCode() { return value.hashCode(); }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let owner = &resolved.program.types[1];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "hashCode" => Some(method),
                _ => None,
            })
            .unwrap();
        assert!(plan.returns_result(&CallableKey::source_method_for_type(owner, method)));
    }

    #[test]
    fn checked_identity_null_effect_uses_solved_shape_and_java_catches() {
        for (body, nullable, expected) in [
            ("return Objects.requireNonNull(value);", false, false),
            (
                "return Objects.requireNonNull(value, \"value\");",
                true,
                true,
            ),
            (
                "try { return Objects.requireNonNull(value); } catch (NullPointerException ex) { return \"fallback\"; }",
                true,
                false,
            ),
        ] {
            let parsed = parse_java_source(&format!(
                "import java.util.Objects; class C {{ static String check(String value) {{ {body} }} }}"
            ));
            let resolved = resolve_program(&parsed.program, parsed.diagnostics);
            let method = resolved.program.types[0]
                .members
                .iter()
                .find_map(|member| {
                    if let Member::Method(method) = member {
                        Some(method)
                    } else {
                        None
                    }
                })
                .unwrap();
            let mut env = crate::expr_shape::ShapeEnv::new();
            let ty = if nullable {
                crate::types::optional_java_type(JavaType::String)
            } else {
                JavaType::String
            };
            env.insert_param_shape_with_java_type(
                "value",
                crate::slot_shape::RustSlotShape::for_storage(ty),
                JavaType::String,
            );
            let signatures = crate::signature::SignaturePlan::default();
            let ctx = crate::expr_shape::ExprShapeContext::new(
                Some(&signatures),
                None,
                Some(&resolved.facts),
            );
            assert_eq!(
                block_has_unhandled_checked_identity_null(
                    method.body.as_ref().unwrap(),
                    &env,
                    ctx,
                    &[],
                    &signatures,
                ),
                expected,
                "{body}, nullable={nullable}"
            );
        }
    }

    #[test]
    fn stored_sam_runtime_effects_use_external_keys_without_shadowing_source_types() {
        for (owner, method, arity) in [
            ("java.util.function.IntSupplier", "getAsInt", 0),
            ("Function", "apply", 1),
            ("Supplier", "get", 0),
        ] {
            let sam = crate::stdlib::functional_interface_shape(
                &crate::stdlib::canonical_external_receiver_type_key(owner),
            )
            .unwrap();
            let key = CallableKey::external_method(Some(owner), sam.result_method_name(), arity);
            assert!(seed_callable_fallibility(&key, &FactSet::default()).returns_result());
            let plain = CallableKey::method_identity(owner, crate::parser::to_snake_case(method));
            assert!(!seed_callable_fallibility(&plain, &FactSet::default()).returns_result());
        }
        let key = CallableKey::external_method(Some("Supplier"), "from_fn", 1);
        assert!(!seed_callable_fallibility(&key, &FactSet::default()).returns_result());
        let parsed = parse_java_source("interface Supplier { int get(); }");
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let key = CallableKey::external_method(Some("Supplier"), "get_result", 0);
        assert!(!seed_callable_fallibility(&key, &resolved.facts).returns_result());
    }

    #[test]
    fn stored_sam_effects_reach_source_callers_but_not_callback_creation() {
        let parsed = parse_java_source(include_str!(
            "../tests/fixtures/retained_callback_state.java"
        ));
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &semantic.facts);
        let plan = plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &semantic.facts,
            Some(&signatures),
        );
        for name in [
            "sharedCounter",
            "localCounter",
            "readStored",
            "held",
            "invokeReturned",
        ] {
            let method = resolved.program.types[0]
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == name => Some(method),
                    _ => None,
                })
                .unwrap();
            assert!(
                plan.returns_result(&CallableKey::source_method_for_type(
                    &resolved.program.types[0],
                    method
                )),
                "{name}"
            );
        }
        for name in ["counter", "indexed", "plain", "reference"] {
            let method = resolved.program.types[0]
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == name => Some(method),
                    _ => None,
                })
                .unwrap();
            assert!(
                !plan.returns_result(&CallableKey::source_method_for_type(
                    &resolved.program.types[0],
                    method
                )),
                "creation: {name}"
            );
        }
    }

    #[test]
    fn source_callback_method_references_resolve_zero_bound_and_unbound_inputs() {
        let parsed = parse_java_source(
            r#"
            import java.util.function.BooleanSupplier;
            import java.util.function.IntBinaryOperator;
            import java.util.function.BiPredicate;
            class Routes {
                static boolean zero(BooleanSupplier callback) { return callback.getAsBoolean(); }
                static int bound(IntBinaryOperator callback) { return callback.applyAsInt(1, 2); }
                static boolean unbound(BiPredicate<Worker, Worker> callback, Worker worker) { return callback.test(worker, worker); }
                static boolean checkedZero() { throw new IllegalArgumentException(); }
                static boolean useZero() { return zero(Routes::checkedZero); }
                static int useBound(Worker worker) { return bound(worker::pair); }
                static boolean useUnbound(Worker worker) { return unbound(Worker::same, worker); }
            }
            class Worker {
                int pair(int a, int b) { throw new IllegalArgumentException(); }
                boolean same(Worker other) { throw new IllegalArgumentException(); }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let callbacks =
            crate::source_callbacks::SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let graph = crate::usage::build_call_graph_with_source_callbacks(
            &resolved.program,
            &resolved.facts,
            &callbacks,
        );
        let plan = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let worker = JavaType::Named {
            name: "Worker".to_string(),
            args: Vec::new(),
        };
        let source_symbols = crate::resolve::source_symbols_from_program(&resolved.program);
        assert!(
            crate::resolve::resolve_source_method_reference_with_input_types(
                "Worker",
                "same",
                &[worker.clone(), worker],
                "Routes",
                &source_symbols,
                &crate::types::TypeEnv::new(),
                &resolved.facts,
            )
            .is_some(),
            "unbound resolution"
        );
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            if !matches!(method.name.as_str(), "zero" | "bound" | "unbound") {
                continue;
            }
            assert!(callbacks.is_inline(owner, method, 0), "{}", method.name);
            assert!(
                plan.returns_result(&CallableKey::CallbackParameter {
                    owner: Box::new(CallableKey::source_method_for_type(owner, method)),
                    index: 0,
                }),
                "{}: {graph:#?}",
                method.name
            );
        }
    }

    #[test]
    fn enhanced_for_null_receiver_is_an_effect_unless_caught() {
        let parsed = parse_java_source(
            r#"
            class Loops {
                static int count(Object[] values) {
                    for (Object value : values) { }
                    return 1;
                }
                static int caught(Object[] values) {
                    try { for (Object value : values) { } }
                    catch (NullPointerException expected) { return -1; }
                    return 1;
                }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &resolved.facts,
            None,
        );
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            if let Member::Method(method) = member {
                assert_eq!(
                    plan.returns_result(&CallableKey::source_method_for_type(owner, method)),
                    method.name == "count",
                    "{}",
                    method.name
                );
            }
        }
    }

    #[test]
    fn source_sam_bodies_share_effects_without_executing_at_creation() {
        let parsed = parse_java_source(
            r#"
            interface Action<T> { int get(T value); }
            interface Clean { int get(); }
            class Routes {
                static int invoke(Action<String> action) { return action.get("value"); }
                static int unused(Action<String> action) { return 3; }
                static int failing() { return invoke(value -> { throw new IllegalArgumentException(); }); }
                static int plain() { return invoke(value -> value.length()); }
                static int creationOnly() { return unused(value -> { throw new IllegalArgumentException(); }); }
                static int constant(String value) { return 4; }
                static int reference() { return invoke(Routes::constant); }
                static int clean(Clean action) { return action.get(); }
                static int cleanUse() { return clean(() -> 4); }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let plan = plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &resolved.facts,
            None,
        );
        for owner in &resolved.program.types {
            for member in &owner.members {
                let Member::Method(method) = member else {
                    continue;
                };
                let expected = owner.name == "Action"
                    || matches!(
                        method.name.as_str(),
                        "invoke" | "failing" | "plain" | "reference" | "clean" | "cleanUse"
                    );
                assert_eq!(
                    plan.returns_result(&CallableKey::source_method_for_type(owner, method)),
                    expected,
                    "{}.{}",
                    owner.name,
                    method.name,
                );
            }
        }
        let bodies: Vec<_> = graph.synthetic_callable_bodies().collect();
        assert_eq!(bodies.len(), 4);
        assert_eq!(
            bodies
                .iter()
                .filter(|(key, _, _)| plan.returns_result(key))
                .count(),
            3
        );
    }

    #[test]
    fn source_callback_contracts_share_the_existing_fallibility_solver() {
        let parsed = parse_java_source(
            r#"
            import java.util.function.IntPredicate;
            import java.util.function.IntUnaryOperator;
            import java.util.function.IntBinaryOperator;
            class Routes {
                static boolean check(IntPredicate predicate, int value) { return predicate.test(value); }
                static boolean forward(IntPredicate predicate, int value) { return check(predicate, value); }
                static boolean recursive(IntPredicate predicate, int value) {
                    if (value <= 0) return predicate.test(value);
                    return recursive(predicate, value - 1);
                }
                static boolean checked(int value) {
                    if (value < 0) throw new IllegalArgumentException();
                    return value > 0;
                }
                static boolean risky(int value) { return forward(item -> checked(item), value); }
                static boolean reference(int value) { return check(Routes::checked, value); }
                static boolean plain(int value) { return check(item -> item > 0, value); }
                static boolean recursiveUse(int value) { return recursive(Routes::checked, value); }
                static int unused(IntPredicate predicate) { return 1; }
                static int unusedThrow() { return unused(item -> { throw new IllegalArgumentException(); }); }
                static boolean independent(IntPredicate clean, int value) { return clean.test(value); }
                static int apply(IntUnaryOperator operator, int value) { return operator.applyAsInt(value); }
                static int divide(int value) { return apply(item -> 1 / item, value); }
                static int binary(IntBinaryOperator operator, int a, int b) { return operator.applyAsInt(a, b); }
                static int checkedPair(int a, int b) { if (a < b) throw new IllegalArgumentException(); return a; }
                static String checkedPair(String a, String b) { return a; }
                static int binaryReference(int a, int b) { return binary(Routes::checkedPair, a, b); }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let callbacks =
            crate::source_callbacks::SourceCallbackPlan::build(&resolved.program, &resolved.facts);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let graph = crate::usage::build_call_graph_with_source_callbacks(
            &resolved.program,
            &resolved.facts,
            &callbacks,
        );
        let plan = plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &semantic.facts,
            Some(&signatures),
        );
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            let expected = matches!(
                method.name.as_str(),
                "check"
                    | "forward"
                    | "recursive"
                    | "checked"
                    | "risky"
                    | "reference"
                    | "plain"
                    | "recursiveUse"
                    | "apply"
                    | "divide"
                    | "binary"
                    | "binaryReference"
            ) || (method.name == "checkedPair"
                && method.params[0].ty == JavaType::Primitive(PrimitiveType::Int));
            assert_eq!(
                plan.returns_result(&CallableKey::source_method_for_type(owner, method)),
                expected,
                "{} {:?} {:?}",
                method.name,
                method.params,
                plan.callables
                    .get(&CallableKey::source_method_for_type(owner, method))
            );
            if callbacks.is_inline(owner, method, 0) {
                assert_eq!(
                    plan.returns_result(&CallableKey::CallbackParameter {
                        owner: Box::new(CallableKey::source_method_for_type(owner, method)),
                        index: 0,
                    }),
                    method.name != "independent",
                    "callback {}",
                    method.name
                );
            }
        }
        let unused = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "unused" => Some(method),
                _ => None,
            })
            .unwrap();
        // Its actual body can throw, but no source invocation executes it.
        assert!(plan.returns_result(&CallableKey::CallbackParameter {
            owner: Box::new(CallableKey::source_method_for_type(owner, unused)),
            index: 0,
        }));
        assert_eq!(
            graph
                .synthetic_callable_bodies()
                .filter(|(key, _, _)| key.is_lambda())
                .count(),
            4
        );
    }

    #[test]
    fn returned_and_stored_source_sams_share_invocation_abi_without_executing_at_creation() {
        let parsed = parse_java_source(
            r#"
            interface TextFunction { int apply(String value); }
            class Factory {
                static int checked(String value) {
                    if (value == null) throw new IllegalArgumentException();
                    return value.length();
                }
                static int checked(int value) { return value + 1; }
                static TextFunction returned() { return value -> checked(value); }
                static TextFunction referenced() { return Factory::checked; }
                static TextFunction stored() { TextFunction value = item -> checked(item); return value; }
                static TextFunction assigned() { TextFunction value = null; value = Factory::checked; return value; }
                static TextFunction branch(boolean choose) { return choose ? item -> checked(item) : Factory::checked; }
                static TextFunction clean() { return value -> 7; }
                static int use() { return returned().apply("x"); }
            }
        "#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let graph = crate::usage::build_call_graph_with_source_callbacks(
            &resolved.program,
            &resolved.facts,
            signatures.source_callback_plan(),
        );
        let plan = plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &semantic.facts,
            Some(&signatures),
        );
        for owner in &resolved.program.types {
            for member in &owner.members {
                let Member::Method(method) = member else {
                    continue;
                };
                let expected = method.name == "apply"
                    || method.name == "use"
                    || (method.name == "checked" && method.params[0].ty == JavaType::String);
                let key = CallableKey::source_method_for_type(owner, method);
                assert_eq!(plan.returns_result(&key), expected, "{key:?}: {:?}", plan);
            }
        }
    }

    fn test_type(name: &str) -> TypeDecl {
        TypeDecl {
            source_names: None,
            kind: TypeKind::Class,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn test_method(java_name: &str, rust_name: Option<&str>) -> MethodDecl {
        MethodDecl {
            name: java_name.to_string(),
            rust_name: rust_name.map(str::to_string),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Void,
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: vec![Modifier::Public],
            body: None,
            span: Span::default(),
        }
    }

    fn qualified_new_expr(qualifier: Expr) -> Expr {
        Expr::New {
            ty: JavaType::Unknown("Inner".to_string()),
            infer_type_arguments: false,
            qualifier: Some(Box::new(qualifier)),
            args: Vec::new(),
            body: None,
            resolved: None,
        }
    }

    #[test]
    fn integer_div_rem_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(Expr::Binary {
            op: BinaryOp::Div,
            left: Box::new(Expr::Literal(Literal::Int(4))),
            right: Box::new(Expr::Literal(Literal::Int(2))),
        });

        assert!(expr_has_integer_div_rem_expr(&expr));
    }

    #[test]
    fn integer_div_rem_scan_sees_array_index() {
        let expr = Expr::ArrayAccess(
            Box::new(Expr::Var("items".to_string())),
            Box::new(Expr::Binary {
                op: BinaryOp::Rem,
                left: Box::new(Expr::Literal(Literal::Int(5))),
                right: Box::new(Expr::Literal(Literal::Int(2))),
            }),
        );

        assert!(expr_has_integer_div_rem_expr(&expr));
    }

    #[test]
    fn checked_native_arithmetic_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(Expr::Var("Math".to_string()))),
            name: "toIntExact".to_string(),
            args: vec![Expr::Var("value".to_string())],
            resolved: None,
        });

        assert!(expr_has_checked_native_arithmetic_call(&expr));
    }

    #[test]
    fn nullable_receiver_deref_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(Expr::FieldAccess(
            Box::new(Expr::Var("maybe".to_string())),
            "value".to_string(),
        ));
        let names = HashSet::from(["maybe".to_string()]);

        assert!(expr_has_nullable_receiver_deref(&expr, &names));
    }

    #[test]
    fn method_plan_uses_only_canonical_source_method_key() {
        let ty = test_type("Owner");
        let method = test_method("camelCase", Some("camel_case"));
        let mut plan = ProgramExceptionPlan::default();
        plan.methods.insert(
            ("Owner".to_string(), "camelCase".to_string()),
            MethodExceptionPlan::new(true),
        );

        assert_eq!(
            plan.method_plan(&ty, &method),
            MethodExceptionPlan::new(false)
        );

        plan.methods.insert(
            method_exception_key(&ty, &method),
            MethodExceptionPlan::new(true),
        );
        assert_eq!(
            plan.method_plan(&ty, &method),
            MethodExceptionPlan::new(true)
        );
    }

    #[test]
    fn intrinsic_exception_handlers_share_jdk_subtype_rules() {
        let checks: [(fn(&[CatchClause]) -> bool, &str); 4] = [
            (catches_arithmetic_exception, "ArithmeticException"),
            (
                catches_array_bounds_exception,
                "ArrayIndexOutOfBoundsException",
            ),
            (catches_class_cast_exception, "ClassCastException"),
            (catches_null_pointer_exception, "NullPointerException"),
        ];
        let handler = |name: &str| CatchClause {
            types: vec![JavaType::Named {
                name: name.to_string(),
                args: vec![],
            }],
            name: "error".to_string(),
            source_name: "error".to_string(),
            body: Block { statements: vec![] },
            binding: None,
        };
        for (matches, leaf) in checks {
            for caught in [leaf, "java.lang.RuntimeException", "Exception", "Throwable"] {
                assert!(matches(&[handler(caught)]), "{leaf} -> {caught}");
            }
            assert!(!matches(&[handler("IOException")]), "{leaf}");
            assert!(!matches(&[handler("Error")]), "{leaf}");
        }
        assert!(catches_array_bounds_exception(&[handler(
            "java.lang.IndexOutOfBoundsException"
        )]));
        assert!(!catches_class_cast_exception(&[handler(
            "IndexOutOfBoundsException"
        )]));
    }

    #[test]
    fn array_bounds_effects_respect_handlers_and_deferred_bodies() {
        let parsed = parse_java_source(
            r#"
class Reads {
    static int direct(int[] values) { return values[0] + 2; }
    static void write(int[] values) { values[1] = 3; }
    static int handled(int[] values) {
        try { return values[0]; }
        catch (IndexOutOfBoundsException error) { return -1; }
    }
    static int otherHandler(int[] values) {
        try { return values[0]; }
        catch (ArithmeticException error) { return -1; }
    }
    static Object deferred(int[] values) {
        return new Object() { int read() { return values[0]; } };
    }
    static Runnable lambda(int[] values) {
        return () -> { int value = values[0]; };
    }
    static Object immediate(int[] values) {
        return new Object() { int value = values[0]; };
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let keys = array_bounds_throw_callables(&resolved.program);
        let ty = &resolved.program.types[0];
        for member in &ty.members {
            if let Member::Method(method) = member {
                assert_eq!(
                    keys.contains(&CallableKey::source_method_for_type(ty, method)),
                    matches!(
                        method.name.as_str(),
                        "direct" | "write" | "otherHandler" | "immediate"
                    ),
                    "{}",
                    method.name
                );
            }
        }
    }

    #[test]
    fn checked_cast_effects_use_conversion_plans_and_scoped_handlers() {
        let parsed = parse_java_source(
            r#"
class Casts {
    static String checked(Object value) { return (String) value; }
    static String identity(String value) { return (String) value; }
    static String nullValue() { return (String) null; }
    static long numeric(int value) { return (long) value; }
    static String handled(Object value) {
        try { return (String) value; }
        catch (ClassCastException error) { return "caught"; }
    }
    static String unhandled(Object value) {
        try { return (String) value; }
        catch (IllegalArgumentException error) { return "other"; }
    }
    static String loop() {
        for (String value = "ok"; true;) { return (String) value; }
    }
    static String loopChecked() {
        for (Object value = "ok"; true;) { return (String) value; }
    }
    static String switched(int selector) {
        switch (selector) {
            case 0: String value = "ok"; return (String) value;
            default: return "default";
        }
    }
    static String switchChecked(int selector) {
        switch (selector) {
            case 0: Object value = "ok"; return (String) value;
            default: return "default";
        }
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(!resolved.diagnostics.has_errors());
        let keys = checked_cast_throw_callables(&resolved.program, &FactSet::default(), None, None);
        let ty = &resolved.program.types[0];
        for member in &ty.members {
            let Member::Method(method) = member else {
                continue;
            };
            let expected = matches!(
                method.name.as_str(),
                "checked" | "unhandled" | "loopChecked" | "switchChecked"
            );
            assert_eq!(
                keys.contains(&CallableKey::source_method_for_type(ty, method)),
                expected,
                "{}",
                method.name
            );
        }
    }

    #[test]
    fn checked_cast_in_anonymous_override_promotes_its_contract_not_factory() {
        let parsed = parse_java_source(
            r#"
class Base { String cast(Object value) { return "base"; } }
class Use {
    static Base make() {
        return new Base() {
            @Override String cast(Object value) { return (String) value; }
        };
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = FactSet::default();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let plan = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let anonymous: Vec<_> = call_graph.anonymous_methods().iter().collect();
        assert_eq!(anonymous.len(), 1);
        assert!(plan.returns_result(anonymous[0].0));
        for ty in &resolved.program.types {
            for member in &ty.members {
                if let Member::Method(method) = member {
                    assert_eq!(
                        plan.returns_result(&CallableKey::source_method_for_type(ty, method)),
                        method.name == "cast",
                        "{}",
                        method.name
                    );
                }
            }
        }
    }

    #[test]
    fn shaped_null_checks_respect_fields_shadowing_catches_and_deferred_bodies() {
        let parsed = parse_java_source(
            r#"
import java.util.function.Supplier;
interface Counter { int get(); }
class NullContracts {
    Counter current;
    Integer boxed;
    NullContracts absent;
    static int primitive(int value) { return value; }
    static int safe() { return 1; }
    int bare() { return current.get(); }
    int explicit() { return this.current.get(); }
    int caught() { try { return current.get(); } catch (NullPointerException e) { return 0; } }
    int unbox() { return primitive(boxed); }
    int caughtUnbox() { try { return primitive(boxed); } catch (NullPointerException e) { return 0; } }
    Supplier<Integer> delayed() { return () -> current.get(); }
    int shadow(Counter current) { return current.get(); }
    int staticCall() { return absent.safe(); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let signatures = crate::signature::SignaturePlan::default();
        let ctx = crate::expr_shape::ExprShapeContext::new(
            Some(&signatures),
            None,
            Some(&resolved.facts),
        );
        let mut checked = 0;
        for member in &resolved.program.types[1].members {
            let Member::Method(method) = member else {
                continue;
            };
            let expected = match method.name.as_str() {
                "bare" | "explicit" | "unbox" => true,
                "caught" | "caughtUnbox" | "delayed" | "shadow" | "staticCall" => false,
                _ => continue,
            };
            let mut env = crate::expr_shape::ShapeEnv::new();
            for (name, ty) in [
                ("current", "Counter"),
                ("boxed", "Integer"),
                ("absent", "NullContracts"),
            ] {
                env.insert_field_shape(
                    name,
                    crate::slot_shape::RustSlotShape::for_storage(
                        crate::types::optional_java_type(crate::parser::parse_hierarchy_type(ty)),
                    ),
                );
            }
            if method.name == "shadow" {
                env.insert_local_shape(
                    "current",
                    crate::slot_shape::RustSlotShape::for_storage(
                        crate::parser::parse_hierarchy_type("Counter"),
                    ),
                );
            }
            assert_eq!(
                block_has_unhandled_shaped_call_null_check(
                    method.body.as_ref().unwrap(),
                    &env,
                    ctx,
                    &[],
                    &signatures,
                ),
                expected,
                "{}",
                method.name
            );
            let external_index = resolved.facts.external_lookup_index();
            assert_eq!(
                block_has_unhandled_shaped_call_null_check(
                    method.body.as_ref().unwrap(),
                    &env,
                    ctx.with_external_index(&external_index),
                    &[],
                    &signatures,
                ),
                expected,
                "indexed {}",
                method.name,
            );
            checked += 1;
        }
        assert_eq!(checked, 8);
    }

    #[test]
    fn non_throwing_interface_override_group_stays_infallible() {
        let parsed = parse_java_source(
            r#"
interface Metric {
    int score(String sample);
    void merge(Metric other);
}
class CountMetric implements Metric {
    int value;
    public int score(String sample) { return this.value; }
    public void merge(Metric other) { this.value = this.value + 1; }
}
class FixedMetric implements Metric {
    public int score(String sample) { return 1; }
    public void merge(Metric other) {}
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = FactSet::default();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);

        for (type_name, method_name) in [
            ("Metric", "merge"),
            ("CountMetric", "merge"),
            ("FixedMetric", "merge"),
        ] {
            let ty = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == type_name)
                .expect("test type is present");
            let method = ty
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == method_name => Some(method),
                    _ => None,
                })
                .expect("test method is present");
            let key = CallableKey::source_method_for_type(ty, method);
            assert!(
                !fallibility.returns_result(&key),
                "{key:?}: {:?}",
                fallibility.callable(&key).map(CallableFallibility::reasons)
            );
        }

        let _semantic = analyze_program(&resolved.program, &resolved.facts, &facts);
    }

    #[test]
    fn stream_callback_consumed_references_propagate_errors_but_creation_does_not() {
        let parsed = parse_java_source(
            r#"
import java.util.Arrays;
import java.util.stream.IntStream;
interface IntCheck { boolean test(int value); }
class Callback {
    static boolean checked(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value > 0;
    }
    static boolean checked(String value) { return true; }
    static boolean consumed(int[] values) { return Arrays.stream(values).anyMatch(Callback::checked); }
    static boolean qualified(int[] values) { return Arrays.stream(values).allMatch(value -> Callback.checked(value)); }
    static IntCheck created() { return Callback::checked; }
    static IntStream delayed(int[] values) { return Arrays.stream(values).filter(Callback::checked); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let fallibility = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Callback")
            .unwrap();
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method)
                if matches!(
                    method.name.as_str(),
                    "consumed" | "qualified" | "created" | "delayed"
                ) =>
            {
                Some(method)
            }
            _ => None,
        }) {
            assert_eq!(
                fallibility.returns_result(&CallableKey::source_method_for_type(owner, method)),
                matches!(method.name.as_str(), "consumed" | "qualified"),
                "{}",
                method.name
            );
        }
        let consumed_key = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "consumed" => {
                    Some(CallableKey::source_method_for_type(owner, method))
                }
                _ => None,
            })
            .unwrap();
        let callbacks = graph
            .edges()
            .iter()
            .filter(|edge| {
                edge.caller() == &consumed_key
                    && edge.kind() == crate::usage::CallGraphEdgeKind::LambdaBody
            })
            .map(|edge| edge.callee())
            .collect::<Vec<_>>();
        assert_eq!(callbacks.len(), 1);
        let targets = graph
            .edges()
            .iter()
            .filter(|edge| callbacks.contains(&edge.caller()))
            .map(|edge| edge.callee())
            .collect::<Vec<_>>();
        assert_eq!(
            targets.len(),
            1,
            "a typed unary callback must not collect unrelated overloads"
        );
    }

    #[test]
    fn current_receiver_method_reference_propagates_overload_fallibility() {
        let parsed = parse_java_source(
            r#"
import java.util.List;
import java.util.stream.Collectors;

class Decoder {
    String decode(String value) throws DecodeException {
        throw new DecodeException();
    }

    List<String> decode(List<String> values) {
        return values.stream().map(this::decode).collect(Collectors.toList());
    }
}

class DecodeException extends Exception {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = resolved.facts.clone();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let decoder = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Decoder")
            .expect("Decoder type is present");
        let methods = decoder
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Method(method) if method.name == "decode" => Some(method),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(methods.len(), 2);
        for method in methods {
            let key = CallableKey::source_method_for_type(decoder, method);
            assert!(
                fallibility.returns_result(&key),
                "method-reference overload {key:?} should share callback fallibility; reasons={:?}",
                fallibility.callable(&key).map(CallableFallibility::reasons)
            );
        }
    }

    #[test]
    fn generic_interface_override_group_substitutes_type_arguments() {
        let parsed = parse_java_source(
            r#"
interface Sink<T> {
    void write(T value);
}
class Payload {}
class PayloadSink implements Sink<Payload> {
    public void write(Payload value) throws IOException {
        throw new IOException();
    }

}
class IOException extends Exception {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = resolved.facts.clone();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let override_groups = override_groups_for_program(&resolved.program);

        for type_name in ["PayloadSink", "Sink"] {
            let ty = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == type_name)
                .expect("test type is present");
            let method = ty
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == "write" => Some(method),
                    _ => None,
                })
                .expect("write method is present");
            let key = CallableKey::source_method_for_type(ty, method);
            assert!(
                fallibility.returns_result(&key),
                "{key:?}: {:?}; groups={override_groups:?}",
                fallibility.callable(&key).map(CallableFallibility::reasons)
            );
        }
    }

    #[test]
    fn imported_nested_generic_interface_shares_implementation_fallibility() {
        let parsed = parse_java_source(
            r#"
package sample;
import sample.Scope.Reader;
class Scope {
    interface Reader<T> { T read(T value); }
}
class ValueReader<T> implements Reader<T> {
    public T read(T value) {
        if (value == null) throw new IllegalArgumentException("missing");
        return value;
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = resolved.facts.clone();
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &graph, &facts);
        let groups = override_groups_for_program(&resolved.program);
        let mut types = Vec::new();
        for ty in &resolved.program.types {
            collect_type_decls(ty, &mut types);
        }
        let mut checked = 0;
        for ty in types {
            for member in &ty.members {
                let Member::Method(method) = member else {
                    continue;
                };
                if method.name != "read" {
                    continue;
                }
                checked += 1;
                let key = CallableKey::source_method_for_type(ty, method);
                assert!(
                    fallibility.returns_result(&key),
                    "{key:?}: groups={groups:?}"
                );
                assert!(groups.iter().any(|group| group.contains(&key)), "{key:?}");
            }
        }
        assert_eq!(checked, 2);
    }

    #[test]
    fn nested_override_fallibility_does_not_merge_same_leaf_interfaces() {
        let parsed = parse_java_source(
            r#"
class First { interface Reader { void read(); } }
class Second { interface Reader { void read(); } }
class ValueReader implements First.Reader {
    public void read() { throw new IllegalArgumentException("missing"); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &resolved.facts);
        let fallibility = plan_program_fallibility(&resolved.program, &graph, &resolved.facts);
        let mut types = Vec::new();
        for ty in &resolved.program.types {
            collect_type_decls(ty, &mut types);
        }
        let mut checked = 0;
        for ty in types {
            for member in &ty.members {
                let Member::Method(method) = member else {
                    continue;
                };
                if method.name != "read" {
                    continue;
                }
                checked += 1;
                let key = CallableKey::source_method_for_type(ty, method);
                assert_eq!(
                    fallibility.returns_result(&key),
                    ty.name != "Second.Reader",
                    "{key:?}"
                );
            }
        }
        assert_eq!(checked, 3);
    }

    #[test]
    fn floating_point_division_does_not_make_constructor_fallible() {
        let parsed = parse_java_source(
            r#"
class FloatingRatio {
    static final double SCALE = 0.5;
    double ratio;

    FloatingRatio(int value) {
        ratio = value / SCALE;
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = FactSet::default();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let ty = &resolved.program.types[0];
        let constructor = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Constructor(constructor) => Some(constructor),
                _ => None,
            })
            .expect("constructor is present");
        let key = CallableKey::source_constructor_for_type(ty, constructor);
        assert!(
            !fallibility.returns_result(&key),
            "floating-point division cannot throw ArithmeticException: {:?}",
            fallibility.callable(&key).map(CallableFallibility::reasons)
        );
    }

    #[test]
    fn overloaded_constructor_chain_propagates_private_helper_throw() {
        let parsed = parse_java_source(
            r#"
package fixture;

class Registry {
    static Store<Value> values = new Store<Value>();
    static void register(Value value) { values.add(value); }
    static { register(new Value(2)); }
    static class Store<T> {
        T value;
        void add(T value) { this.value = value; }
    }
}

abstract class Base {
    Base(int divisor) { this(divisor, 0); }
    Base(int divisor, int ignored) { validate(divisor); }
    private void validate(int divisor) {
        if (divisor == 0) { throw new IllegalArgumentException("bad divisor"); }
    }
}

class Value extends Base {
    Value(int divisor) { super(divisor); }
    Value(String value) { super(value.length()); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = resolved.facts.clone();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let value = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Value")
            .expect("Value declaration");
        let constructor = value
            .members
            .iter()
            .find_map(|member| match member {
                Member::Constructor(constructor)
                    if constructor.params.first().is_some_and(|param| {
                        matches!(
                            param.ty,
                            JavaType::Primitive(crate::java_ir::PrimitiveType::Int)
                        )
                    }) =>
                {
                    Some(constructor)
                }
                _ => None,
            })
            .expect("Value(int) constructor");
        let key = CallableKey::source_constructor_for_type(value, constructor);
        assert!(
            fallibility.returns_result(&key),
            "constructor={constructor:#?}\nkey={key:?}\ngraph={call_graph:#?}\nfallibility={fallibility:#?}"
        );
    }

    #[test]
    fn fallible_anonymous_override_promotes_base_contract_not_factory() {
        let parsed = parse_java_source(
            r#"
class Base {
    Base() {}
    int checked(int[] values) { return values[0]; }
    int checked(String ignored) { return -6; }
    int slot(int[] values) { return -1; }
    int direct(int[] values) { return -3; }
    int explicit(boolean fail) { return -9; }
}
class Use {
    static Base make() {
        return new Base() {
            @Override int slot(int[] values) {
                return super.checked(values) + 1;
            }
            @Override int direct(int[] values) { return values[0]; }
            @Override int explicit(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return 7;
            }
        };
    }
    static int run(int[] values) { return make().slot(values); }
    static int runDirect(int[] values) { return make().direct(values); }
    static int runExplicit(boolean fail) { return make().explicit(fail); }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = FactSet::default();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        let method_key = |type_name: &str, method_name: &str| {
            let ty = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == type_name)
                .expect("source type");
            let method = ty
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == method_name => Some(method),
                    _ => None,
                })
                .expect("source method");
            CallableKey::source_method_for_type(ty, method)
        };
        let slot = method_key("Base", "slot");
        let direct = method_key("Base", "direct");
        let explicit = method_key("Base", "explicit");
        let factory = method_key("Use", "make");
        let caller = method_key("Use", "run");
        let direct_caller = method_key("Use", "runDirect");
        let explicit_caller = method_key("Use", "runExplicit");
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let checked_string = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method)
                    if method.name == "checked" && method.params[0].ty == JavaType::String =>
                {
                    Some(CallableKey::source_method_for_type(base, method))
                }
                _ => None,
            })
            .expect("nonthrowing overload");
        let anonymous = call_graph
            .edges()
            .iter()
            .find(|edge| edge.kind() == crate::usage::CallGraphEdgeKind::AnonymousOverrideContract)
            .map(|edge| edge.callee().clone())
            .expect("anonymous override contract edge");

        assert!(fallibility.returns_result(&anonymous));
        assert!(fallibility.returns_result(&slot));
        assert!(fallibility.returns_result(&caller));
        assert!(fallibility.returns_result(&direct));
        assert!(fallibility.returns_result(&direct_caller));
        assert!(fallibility.returns_result(&explicit));
        assert!(fallibility.returns_result(&explicit_caller));
        assert!(!fallibility.returns_result(&factory));
        assert!(!fallibility.returns_result(&checked_string));
    }

    #[test]
    fn fallible_anonymous_duplicate_parent_contracts_share_result_abi() {
        let parsed = parse_java_source(
            r#"
interface LeftCheck { int value(boolean fail); }
interface RightCheck { int value(boolean fail); }
interface BothCheck extends LeftCheck, RightCheck {}
interface GenericEcho<T> { T echo(T input, boolean fail); }
interface StringEcho { String echo(String input, boolean fail); }
interface JoinedEcho extends GenericEcho<String>, StringEcho {}
class Use {
    static BothCheck make() {
        return new BothCheck() {
            int total = 3;
            public int value(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.total;
            }
        };
    }
    static JoinedEcho makeEcho() {
        return new JoinedEcho() {
            int marker = 1;
            public String echo(String input, boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return input;
            }
        };
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let facts = FactSet::default();
        let call_graph = crate::usage::build_call_graph_with_facts(&resolved.program, &facts);
        let fallibility = plan_program_fallibility(&resolved.program, &call_graph, &facts);
        for parent_name in ["LeftCheck", "RightCheck", "GenericEcho", "StringEcho"] {
            let parent = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == parent_name)
                .expect("parent interface");
            let method = parent
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) => Some(method),
                    _ => None,
                })
                .expect("parent contract");
            let key = CallableKey::source_method_for_type(parent, method);
            assert!(
                fallibility.returns_result(&key),
                "{parent_name} stays infallible"
            );
            assert!(call_graph.edges().iter().any(|edge| {
                edge.kind() == crate::usage::CallGraphEdgeKind::AnonymousOverrideContract
                    && edge.caller() == &key
            }));
        }
        let factory = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Use")
            .expect("factory owner");
        for factory_name in ["make", "makeEcho"] {
            let make = factory
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == factory_name => Some(method),
                    _ => None,
                })
                .expect("factory method");
            assert!(
                !fallibility.returns_result(&CallableKey::source_method_for_type(factory, make))
            );
        }
    }

    #[test]
    fn exception_class_name_preserves_unknown_throw_carriers() {
        assert_eq!(
            exception_class_name(&JavaType::Unknown("ExternalException".to_string())),
            "ExternalException"
        );
        assert_eq!(
            exception_class_name(&JavaType::Primitive(PrimitiveType::Int)),
            "JavaException"
        );
    }

    #[test]
    fn throw_expr_plan_preserves_new_existing_and_cause_or_self_shapes() {
        let thrown_new = Expr::New {
            infer_type_arguments: false,
            ty: JavaType::Named {
                name: "IllegalArgumentException".to_string(),
                args: Vec::new(),
            },
            qualifier: None,
            args: vec![Expr::Var("message".to_string())],
            body: None,
            resolved: None,
        };
        assert_eq!(
            plan_throw_expr(&thrown_new),
            ThrowExprPlan::NewException {
                class_name: "IllegalArgumentException".to_string(),
                args: &[Expr::Var("message".to_string())],
            }
        );
        assert_eq!(
            plan_throw_expr(&Expr::Var("err".to_string())),
            ThrowExprPlan::ExistingException(&Expr::Var("err".to_string()))
        );

        let cause_expr = Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(Expr::Var("ce".to_string()))),
            name: "getCause".to_string(),
            args: Vec::new(),
            resolved: None,
        };
        let cause_runtime_rethrow = Expr::Ternary {
            condition: Box::new(Expr::InstanceOf {
                expr: Box::new(cause_expr.clone()),
                ty: JavaType::Named {
                    name: "RuntimeException".to_string(),
                    args: Vec::new(),
                },
                binding: None,
            }),
            then_expr: Box::new(Expr::Cast {
                ty: JavaType::Named {
                    name: "RuntimeException".to_string(),
                    args: Vec::new(),
                },
                expr: Box::new(cause_expr),
            }),
            else_expr: Box::new(Expr::Var("ce".to_string())),
        };
        assert_eq!(
            plan_throw_expr(&cause_runtime_rethrow),
            ThrowExprPlan::CauseOrSelfIfInstance {
                wrapper: &Expr::Var("ce".to_string()),
                class_names: vec!["RuntimeException".to_string()],
            }
        );
    }

    #[test]
    fn catch_match_class_names_uses_semantic_assignable_catch_facts() {
        let source = r#"
class ExceptionSubclassCatchOrder {
    int parse(Source source) {
        try {
            return source.read();
        } catch (SpecificProblem ex) {
            return 1;
        } catch (BaseProblem ex) {
            return 2;
        }
    }
}
class BaseProblem extends Exception {}
class SpecificProblem extends BaseProblem {}
"#;
        let parsed = parse_java_source(source);
        let resolved = resolve_program(&parsed.program, parsed.diagnostics.clone());
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let ty = &parsed.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("expected parse method");
        let Stmt::Try {
            catches: ordered_catches,
            ..
        } = &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("expected ordered try/catch");
        };

        assert_eq!(
            catch_match_class_names(
                Some("ExceptionSubclassCatchOrder"),
                Some("parse"),
                1,
                &ordered_catches[1],
                &semantic.facts,
            ),
            vec!["BaseProblem".to_string(), "SpecificProblem".to_string()]
        );
    }

    #[test]
    fn try_resource_helpers_stay_inside_exception_plan() {
        let resources = vec![
            Stmt::VarDecl {
                ty: JavaType::Unknown("AutoCloseable".to_string()),
                name: "input".to_string(),
                source_name: "input".to_string(),
                initializer: Some(Expr::Literal(Literal::Null)),
                binding: None,
            },
            Stmt::VarDecl {
                ty: JavaType::Unknown("AutoCloseable".to_string()),
                name: "output".to_string(),
                source_name: "output".to_string(),
                initializer: Some(Expr::Literal(Literal::Null)),
                binding: None,
            },
        ];

        assert!(try_resources_are_declarations(&resources));
        assert_eq!(
            try_resource_names(&resources),
            vec!["input".to_string(), "output".to_string()]
        );
        assert!(!try_resources_are_declarations(&[Stmt::Expr(
            Expr::Literal(Literal::Null)
        )]));
    }
}
