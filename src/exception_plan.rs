use std::collections::{BTreeMap, HashMap, HashSet};

use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, Block, CatchClause, Expr, JavaType, Member, MethodDecl, Modifier, Program, Stmt,
    SwitchCase, SwitchLabel, TypeDecl,
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
    /// Integer `/` or `%` can throw ArithmeticException on a zero divisor.
    IntegerDivRemThrow,
    /// Math integer helpers can throw ArithmeticException.
    MathToIntExactThrow,
    /// A nullable Java reference was dereferenced as a receiver and can throw
    /// NullPointerException.
    NullReceiverThrow,
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
        || program_has_math_to_int_exact_call(program)
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
                // Anonymous members have their own parameter/local environments.
                // Keep the older conservative scan for those nested callables.
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
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                expr_has_throwing_integer_div_rem_expr(expr, env)
            }
            crate::java_ir::LambdaBody::Block(block) => block_has_integer_div_rem_expr(block),
        },
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

fn program_has_math_to_int_exact_call(program: &Program) -> bool {
    program.types.iter().any(type_has_math_to_int_exact_call)
}

fn type_has_math_to_int_exact_call(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(block_has_math_to_int_exact_call),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_has_math_to_int_exact_call),
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(expr_has_math_to_int_exact_call),
        Member::Type(nested) => type_has_math_to_int_exact_call(nested),
        Member::Unsupported { .. } => false,
    })
}

fn block_has_math_to_int_exact_call(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_math_to_int_exact_call)
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
        support: plan_java_exception_support(program, facts),
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
    let mut callables = call_graph
        .nodes()
        .iter()
        .cloned()
        .map(|key| {
            let fallibility = seed_callable_fallibility(&key, facts);
            (key, fallibility)
        })
        .collect::<HashMap<_, _>>();
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
    for key in math_to_int_exact_callables(program) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::MathToIntExactThrow);
        }
    }
    for key in nullable_receiver_throw_callables(program, signatures) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::NullReceiverThrow);
        }
    }
    for key in direct_throw_callables(program, facts) {
        let entry = callables.entry(key).or_default();
        if !entry.returns_result {
            entry.returns_result = true;
            entry.reasons.push(FallibilityReason::DirectThrow);
        }
    }
    // Anonymous methods are not members of a named TypeDecl, so the ordinary
    // source-method seed passes above cannot inspect their bodies. The call
    // graph retains their declarations under the same callable identities
    // used by its override-contract edges.
    for (key, method) in call_graph.anonymous_methods() {
        let Some(body) = &method.body else {
            continue;
        };
        let nullable_params = nullable_reference_param_names(&method.params);
        let reasons = [
            (
                block_has_unhandled_integer_div_rem(body),
                FallibilityReason::IntegerDivRemThrow,
            ),
            (
                block_has_unhandled_math_to_int_exact(body),
                FallibilityReason::MathToIntExactThrow,
            ),
            (
                !nullable_params.is_empty()
                    && block_has_unhandled_nullable_receiver_deref(body, &nullable_params),
                FallibilityReason::NullReceiverThrow,
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
                crate::usage::CallGraphEdgeKind::AnonymousOverrideContract => {
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
    catches.iter().any(|catch| {
        catch.types.iter().any(|ty| {
            let name = exception_class_name(ty);
            let simple = name.rsplit('.').next().unwrap_or(&name);
            matches!(
                simple,
                "ArithmeticException" | "RuntimeException" | "Exception" | "Throwable"
            )
        })
    })
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
    block
        .statements
        .iter()
        .any(|stmt| stmt_has_unhandled_integer_div_rem_with_env(stmt, env))
}

fn stmt_has_unhandled_integer_div_rem_with_env(stmt: &Stmt, env: &TypeEnv) -> bool {
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
                    .any(|stmt| stmt_has_unhandled_integer_div_rem_with_env(stmt, env))
                    || block_has_unhandled_integer_div_rem_with_env(body, env)))
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_integer_div_rem_with_env(&catch.body, env))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_integer_div_rem_with_env(block, env))
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_has_throwing_integer_div_rem_expr(expr, env)),
        Stmt::Assign { target, value } => {
            expr_has_throwing_integer_div_rem_expr(target, env)
                || expr_has_throwing_integer_div_rem_expr(value, env)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_has_throwing_integer_div_rem_expr(expr, env),
        Stmt::Return(expr) => expr
            .as_ref()
            .is_some_and(|expr| expr_has_throwing_integer_div_rem_expr(expr, env)),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_throwing_integer_div_rem_expr(condition, env)
                || block_has_unhandled_integer_div_rem_with_env(then_block, env)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_unhandled_integer_div_rem_with_env(block, env))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_has_throwing_integer_div_rem_expr(condition, env)
                || block_has_unhandled_integer_div_rem_with_env(body, env)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_has_unhandled_integer_div_rem_with_env(stmt, env))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_has_throwing_integer_div_rem_expr(expr, env))
                || update
                    .iter()
                    .any(|expr| expr_has_throwing_integer_div_rem_expr(expr, env))
                || block_has_unhandled_integer_div_rem_with_env(body, env)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_throwing_integer_div_rem_expr(iterable, env)
                || block_has_unhandled_integer_div_rem_with_env(body, env)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_integer_div_rem_with_env(body, env)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_throwing_integer_div_rem_expr(selector, env)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => {
                            expr_has_throwing_integer_div_rem_expr(expr, env)
                        }
                        SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_has_unhandled_integer_div_rem_with_env(stmt, env))
                })
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_has_throwing_integer_div_rem_expr(expr, env)),
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

fn math_to_int_exact_callables(program: &Program) -> Vec<CallableKey> {
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
                        .is_some_and(block_has_unhandled_math_to_int_exact)
                    {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    if constructor
                        .body
                        .as_ref()
                        .is_some_and(block_has_unhandled_math_to_int_exact)
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

fn block_has_unhandled_math_to_int_exact(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(stmt_has_unhandled_math_to_int_exact)
}

fn stmt_has_unhandled_math_to_int_exact(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let guarded_counts = !catches_arithmetic_exception(catches);
            (guarded_counts
                && (resources.iter().any(stmt_has_unhandled_math_to_int_exact)
                    || block_has_unhandled_math_to_int_exact(body)))
                || catches
                    .iter()
                    .any(|catch| block_has_unhandled_math_to_int_exact(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_math_to_int_exact)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_math_to_int_exact_call(condition)
                || block_has_unhandled_math_to_int_exact(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_has_unhandled_math_to_int_exact)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_has_math_to_int_exact_call(condition)
                || block_has_unhandled_math_to_int_exact(body)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_has_unhandled_math_to_int_exact)
                || condition
                    .as_ref()
                    .is_some_and(expr_has_math_to_int_exact_call)
                || update.iter().any(expr_has_math_to_int_exact_call)
                || block_has_unhandled_math_to_int_exact(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_math_to_int_exact_call(iterable) || block_has_unhandled_math_to_int_exact(body)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_has_unhandled_math_to_int_exact(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_math_to_int_exact_call(selector)
                || cases.iter().any(|case| {
                    case.labels
                        .iter()
                        .any(switch_label_has_math_to_int_exact_call)
                        || case
                            .statements
                            .iter()
                            .any(stmt_has_unhandled_math_to_int_exact)
                })
        }
        _ => stmt_has_math_to_int_exact_call(stmt),
    }
}

fn stmt_has_math_to_int_exact_call(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(expr_has_math_to_int_exact_call),
        Stmt::Assign { target, value } => {
            expr_has_math_to_int_exact_call(target) || expr_has_math_to_int_exact_call(value)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) => expr_has_math_to_int_exact_call(expr),
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
        Stmt::Throw(expr) => expr_has_math_to_int_exact_call(expr),
        _ => false,
    }
}

fn switch_label_has_math_to_int_exact_call(label: &crate::java_ir::SwitchLabel) -> bool {
    match label {
        crate::java_ir::SwitchLabel::Default => false,
        crate::java_ir::SwitchLabel::Expr(expr) => expr_has_math_to_int_exact_call(expr),
    }
}

fn expr_has_math_to_int_exact_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            call_is_math_to_int_exact(receiver.as_deref(), name, args.len())
                || receiver
                    .as_ref()
                    .is_some_and(|receiver| expr_has_math_to_int_exact_call(receiver))
                || args.iter().any(expr_has_math_to_int_exact_call)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_math_to_int_exact_call(condition)
                || expr_has_math_to_int_exact_call(then_expr)
                || expr_has_math_to_int_exact_call(else_expr)
        }
        Expr::Switch { selector, cases } => {
            expr_has_math_to_int_exact_call(selector)
                || cases.iter().any(|case| {
                    expr_has_math_to_int_exact_call(&case.value)
                        || case
                            .labels
                            .iter()
                            .any(switch_label_has_math_to_int_exact_call)
                })
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => expr_has_math_to_int_exact_call(receiver),
        Expr::Binary { left, right, .. } => {
            expr_has_math_to_int_exact_call(left) || expr_has_math_to_int_exact_call(right)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_has_math_to_int_exact_call(qualifier))
                || args.iter().any(expr_has_math_to_int_exact_call)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(block_has_math_to_int_exact_call),
                        Member::Constructor(constructor) => constructor
                            .body
                            .as_ref()
                            .is_some_and(block_has_math_to_int_exact_call),
                        Member::Field(field) => field
                            .initializer
                            .as_ref()
                            .is_some_and(expr_has_math_to_int_exact_call),
                        Member::Type(nested) => type_has_math_to_int_exact_call(nested),
                        Member::Unsupported { .. } => false,
                    })
                })
        }
        Expr::ArrayInitializer(args) => args.iter().any(expr_has_math_to_int_exact_call),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_has_math_to_int_exact_call)
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(expr_has_math_to_int_exact_call))
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

fn call_is_math_to_int_exact(receiver: Option<&Expr>, name: &str, arg_count: usize) -> bool {
    (matches!(name, "toIntExact" | "to_int_exact") && arg_count == 1
        || matches!(name, "floorDiv" | "floor_div" | "floorMod" | "floor_mod") && arg_count == 2)
        && receiver.is_none_or(expr_is_math_static_owner)
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

fn nullable_receiver_throw_callables(
    program: &Program,
    signatures: Option<&crate::signature::SignaturePlan>,
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
                            nullable_reference_param_names_with_shapes(
                                &method.params,
                                signature.params().iter().map(|param| param.shape()),
                            )
                        })
                        .unwrap_or_else(|| nullable_reference_param_names(&method.params));
                    if !nullable_params.is_empty()
                        && method.body.as_ref().is_some_and(|body| {
                            block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                        })
                    {
                        keys.push(CallableKey::source_method_for_type(ty, method));
                    }
                }
                Member::Constructor(constructor) => {
                    let nullable_params = signatures
                        .and_then(|signatures| signatures.constructor_signature(ty, constructor))
                        .map(|signature| {
                            nullable_reference_param_names_with_shapes(
                                &constructor.params,
                                signature.params().iter().map(|param| param.shape()),
                            )
                        })
                        .unwrap_or_else(|| nullable_reference_param_names(&constructor.params));
                    if !nullable_params.is_empty()
                        && constructor.body.as_ref().is_some_and(|body| {
                            block_has_unhandled_nullable_receiver_deref(body, &nullable_params)
                        })
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

fn nullable_reference_param_names_with_shapes<'a>(
    params: &[crate::java_ir::Param],
    shapes: impl IntoIterator<Item = &'a crate::slot_shape::RustSlotShape>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for (param, shape) in params.iter().zip(shapes) {
        if java_type_can_be_null(&param.ty) && shape.optional_element_type().is_some() {
            names.insert(param.name.clone());
            names.insert(param.source_name.clone());
        }
    }
    names
}

fn java_type_can_be_null(ty: &JavaType) -> bool {
    !matches!(ty, JavaType::Void | JavaType::Primitive(_))
}

fn catches_null_pointer_exception(catches: &[CatchClause]) -> bool {
    catches.iter().any(|catch| {
        catch.types.iter().any(|ty| {
            let name = exception_class_name(ty);
            let simple = name.rsplit('.').next().unwrap_or(&name);
            matches!(
                simple,
                "NullPointerException" | "RuntimeException" | "Exception" | "Throwable"
            )
        })
    })
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
            expr_has_nullable_receiver_deref(iterable, names)
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
    let by_name = types
        .iter()
        .map(|ty| (ty.name.as_str(), *ty))
        .collect::<HashMap<_, _>>();
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
            for (supertype, super_method) in supertype_methods_matching_slot(ty, method, &by_name) {
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
    by_name: &HashMap<&str, &'a TypeDecl>,
) -> Vec<(String, &'a MethodDecl)> {
    let mut matches = Vec::new();
    let wanted = JavaCallableSlotKey::source_method(method);
    let mut pending = ty
        .extends
        .iter()
        .chain(ty.implements.iter())
        .map(|name| crate::parser::parse_hierarchy_type(name))
        .collect::<Vec<_>>();
    let mut seen = std::collections::HashSet::new();
    while let Some(super_ref) = pending.pop() {
        let JavaType::Named { name, args } = super_ref else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(supertype) = by_name.get(name.as_str()) else {
            continue;
        };
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
                    crate::resolve::substitute_method_type_variables(&parent, &substitutions)
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
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::{Literal, Modifier, PrimitiveType, TypeKind};
    use crate::parser::parse_java_source;
    use crate::resolve::resolve_program;
    use crate::semantic_analysis::analyze_program;

    fn test_type(name: &str) -> TypeDecl {
        TypeDecl {
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
    fn math_to_int_exact_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(Expr::Call {
            receiver: Some(Box::new(Expr::Var("Math".to_string()))),
            name: "toIntExact".to_string(),
            args: vec![Expr::Var("value".to_string())],
            resolved: None,
        });

        assert!(expr_has_math_to_int_exact_call(&expr));
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
