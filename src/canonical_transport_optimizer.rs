//! First bounded optimization slice: private scalar observers can borrow a
//! canonical Java handle without changing allocation, dispatch or result ABI.
//! Eligibility is deliberately a proof of an existing lowering recipe, not a
//! prediction that an arbitrary borrowed signature will compile.
use std::collections::{HashMap, HashSet};

use crate::{
    facts::FactSet,
    java_ir::{
        Block, Expr, JavaType, Member, MethodDecl, Modifier, Program, Stmt, TypeDecl, UnaryOp,
    },
    liveness::LivenessAnalysis,
    representation::RepresentationPlan,
    representation_cost::CostWeights,
    representation_solver::{Budget, Problem, Relation, Statistics, Variable},
    signature::{MethodKey, MethodSignaturePlan, SignaturePlan},
    slot_shape::{BorrowTarget, DispatchShape, OwnershipShape, RustSlotShape, ViewShape},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TransportDecision {
    pub method: MethodKey,
    pub parameter: usize,
    pub calls: usize,
    pub old_cost: u128,
    pub selected_cost: u128,
    pub borrowed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TransportOptimizationReport {
    pub decisions: Vec<TransportDecision>,
    pub rejections: Vec<(MethodKey, &'static str)>,
    pub unsupported: usize,
    pub statistics: Statistics,
}

struct Candidate {
    key: MethodKey,
    java_name: String,
    original: RustSlotShape,
    borrowed: RustSlotShape,
    expected_calls: usize,
    observed_calls: usize,
    owned_cost: u64,
    borrowed_cost: u64,
    field_copy_sites: u64,
    rejected: Option<&'static str>,
    sync: bool,
}

fn types<'a>(items: &'a [TypeDecl], out: &mut Vec<&'a TypeDecl>) {
    for ty in items {
        out.push(ty);
        for member in &ty.members {
            if let Member::Type(child) = member {
                types(std::slice::from_ref(child), out);
            }
        }
    }
}

/// No callback, retention, mutation, payload escape or parameter rebinding is
/// expressible in this proof domain. All field observations are primitive.
fn scalar_observer(expr: &Expr, param: &str, object: &TypeDecl) -> bool {
    match expr {
        Expr::Literal(literal) => matches!(literal,
            crate::java_ir::Literal::Bool(_) | crate::java_ir::Literal::Int(_) |
            crate::java_ir::Literal::Long(_) | crate::java_ir::Literal::Float(_) |
            crate::java_ir::Literal::Double(_) | crate::java_ir::Literal::Char(_)),
        Expr::FieldAccess(base, name) if matches!(base.as_ref(), Expr::Var(n) if n == param) => {
            object.members.iter().any(|member| matches!(member, Member::Field(field)
                if field.name == *name && matches!(field.ty, JavaType::Primitive(_)) && !field.modifiers.contains(&Modifier::Static)))
        }
        Expr::Binary { left, right, .. } => scalar_observer(left, param, object) && scalar_observer(right, param, object),
        Expr::Unary { op: UnaryOp::Not | UnaryOp::BitNot | UnaryOp::Neg | UnaryOp::Pos, expr } => scalar_observer(expr, param, object),
        Expr::Ternary { condition, then_expr, else_expr } => scalar_observer(condition, param, object) && scalar_observer(then_expr, param, object) && scalar_observer(else_expr, param, object),
        _ => false,
    }
}

fn candidate(
    owner: &TypeDecl,
    method: &MethodDecl,
    all_types: &[&TypeDecl],
    representations: &RepresentationPlan,
    facts: &FactSet,
    plan: &SignaturePlan,
) -> Option<Candidate> {
    if !owner.type_params.is_empty()
        || !method.type_params.is_empty()
        || method.params.len() != 1
        || !method.modifiers.contains(&Modifier::Private)
        || !method.modifiers.contains(&Modifier::Static)
        || method.modifiers.contains(&Modifier::Synchronized)
        || !matches!(method.return_type, JavaType::Primitive(_))
    {
        return None;
    }
    let signature = plan.method_signature(owner, method)?;
    let param = &signature.params()[0];
    let original = param.shape();
    if param.is_varargs()
        || param.binding_mutable()
        || original.optional_element_type().is_some()
        || original.ownership != OwnershipShape::Owned
        || original.dispatch != DispatchShape::Concrete
        || original.view != ViewShape::Native
        || original.native_allocation.shared()
    {
        return None;
    }
    let JavaType::Named { name, args } = &original.java_ty else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let matches = all_types
        .iter()
        .filter(|ty| ty.name == *name)
        .copied()
        .collect::<Vec<_>>();
    let [object] = matches.as_slice() else {
        return None;
    };
    if !object.type_params.is_empty() {
        return None;
    }
    let placement = crate::storage_strategy::effective_storage_placement(
        facts.storage_placement_for(name),
        representations.storage_placement_for(name),
    );
    if !placement.shared() || !placement.is_implemented() {
        return None;
    }
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(expr))] = body.statements.as_slice() else {
        return None;
    };
    if !scalar_observer(expr, param.name(), object) {
        return None;
    }
    let mut borrowed = original.clone();
    borrowed.ownership = OwnershipShape::SharedBorrow;
    borrowed.borrow_target = BorrowTarget::Carrier;
    // Check the actual conversion recipe rather than merely comparing labels.
    let conversion = crate::conversion_plan::plan_slot_conversion(
        original.clone(),
        crate::conversion_plan::Slot::Argument(borrowed.clone()),
        facts,
    );
    if !conversion.conversion().unsupported_diagnostics().is_empty() {
        return None;
    }
    let mut field_copy_sites = 0;
    if placement.interior_mutable() {
        crate::semantic_analysis::walk_expr(expr, &mut |expr| {
            if matches!(expr, Expr::FieldAccess(base, _) if matches!(base.as_ref(), Expr::Var(n) if n == param.name()))
            {
                field_copy_sites += 1;
            }
        });
    }
    Some(Candidate {
        key: MethodKey::source_method(owner, method),
        java_name: method.name.clone(),
        original: original.clone(),
        borrowed,
        expected_calls: 0,
        observed_calls: 0,
        owned_cost: 0,
        borrowed_cost: 0,
        field_copy_sites,
        rejected: None,
        sync: placement.sync(),
    })
}

fn unique_shape<'a>(signature: &'a MethodSignaturePlan, name: &str) -> Option<&'a RustSlotShape> {
    let mut shapes = signature
        .params()
        .iter()
        .filter(|p| p.name() == name)
        .map(|p| p.shape())
        .chain(
            signature
                .locals()
                .iter()
                .filter(|p| p.name() == name)
                .map(|p| p.shape()),
        );
    let shape = shapes.next()?;
    if shapes.next().is_some() {
        None
    } else {
        Some(shape)
    }
}

fn statement_expressions(stmt: &Stmt) -> Vec<&Expr> {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        }
        | Stmt::Return(Some(expr))
        | Stmt::Expr(expr) => vec![expr],
        Stmt::Assign { target, value } => vec![target, value],
        _ => vec![],
    }
}

fn observe_body(
    body: &Block,
    signature: &MethodSignaturePlan,
    indices: &HashMap<MethodKey, usize>,
    candidates: &mut [Candidate],
    weights: &CostWeights,
) {
    let mut relevant = false;
    for stmt in &body.statements {
        let exprs = statement_expressions(stmt);
        for expr in exprs {
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
                if let Expr::Call {
                    resolved: Some(resolved),
                    ..
                } = expr
                {
                    relevant |= indices.contains_key(&MethodKey::resolved_call(resolved));
                }
            });
        }
    }
    if !relevant {
        return;
    }
    let liveness = LivenessAnalysis::for_body(body);
    // Unsupported nested statements still count in expected_calls, pinning the
    // affected ABI. This avoids inventing scope or callback lifetime evidence.
    for stmt in &body.statements {
        let expressions = statement_expressions(stmt);
        let counts = liveness.use_counts_in(stmt);
        for expr in expressions {
            crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |expr| {
                let Expr::Call {
                    args,
                    resolved: Some(resolved),
                    ..
                } = expr
                else {
                    return;
                };
                let Some(&index) = indices.get(&MethodKey::resolved_call(resolved)) else {
                    return;
                };
                let candidate = &mut candidates[index];
                candidate.observed_calls += 1;
                let [Expr::Var(name)] = args.as_slice() else {
                    candidate.rejected = Some("non_variable_actual");
                    return;
                };
                let Some(actual) = unique_shape(signature, name) else {
                    candidate.rejected = Some("missing_or_ambiguous_actual_shape");
                    return;
                };
                if actual.java_ty != candidate.original.java_ty
                    || actual.dispatch != candidate.original.dispatch
                    || actual.view != candidate.original.view
                    || actual.optional_element_type().is_some()
                    || !matches!(
                        actual.ownership,
                        OwnershipShape::Owned | OwnershipShape::SharedBorrow
                    )
                    || (actual.ownership == OwnershipShape::SharedBorrow
                        && actual.borrow_target != BorrowTarget::Carrier)
                {
                    candidate.rejected = Some("incompatible_carrier_shape");
                    return;
                }
                let Some(live_after) = liveness.binding_is_live_after(stmt, name) else {
                    candidate.rejected = Some("missing_liveness");
                    return;
                };
                // Storage is fixed. Only reference copies saved at this exact
                // call and loans introduced there contribute to the delta.
                let sync = candidate.sync;
                let copy = actual.ownership == OwnershipShape::SharedBorrow
                    || live_after
                    || counts.get(name).copied().unwrap_or(0) > 1;
                if copy {
                    candidate.owned_cost +=
                        weights.0[if sync { "op.arc_clone" } else { "op.rc_clone" }];
                }
                candidate.borrowed_cost += weights.0["type.borrow"];
            });
        }
    }
}

pub(crate) fn optimize(
    program: &Program,
    representations: &RepresentationPlan,
    facts: &FactSet,
    plan: &mut SignaturePlan,
) -> TransportOptimizationReport {
    let mut declarations = Vec::new();
    types(&program.types, &mut declarations);
    let mut candidates = declarations
        .iter()
        .flat_map(|owner| {
            owner.members.iter().filter_map(|m| {
                if let Member::Method(method) = m {
                    candidate(owner, method, &declarations, representations, facts, plan)
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();
    let mut report = TransportOptimizationReport::default();
    if candidates.is_empty() {
        return report;
    }
    let indices = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (c.key.clone(), i))
        .collect::<HashMap<_, _>>();
    // Ambiguous method references and unresolved calls are eligibility limits,
    // not evidence of non-use. Pin candidates with matching Java names.
    let mut pinned_names = HashSet::new();
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| match expr {
        Expr::MethodReference { method, .. } => {
            pinned_names.insert(method.clone());
        }
        Expr::Call {
            name,
            resolved: None,
            ..
        } => {
            pinned_names.insert(name.clone());
        }
        Expr::Call {
            resolved: Some(resolved),
            ..
        } => {
            if let Some(&i) = indices.get(&MethodKey::resolved_call(resolved)) {
                candidates[i].expected_calls += 1;
            }
        }
        _ => {}
    });
    let weights = CostWeights::default();
    for owner in &declarations {
        for member in &owner.members {
            if let Member::Method(method) = member {
                if let (Some(body), Some(signature)) =
                    (method.body.as_ref(), plan.method_signature(owner, method))
                {
                    observe_body(body, signature, &indices, &mut candidates, &weights);
                }
            }
        }
    }
    let mut problem = Problem::default();
    let mut admitted = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.rejected.is_some()
            || pinned_names.contains(&candidate.java_name)
            || candidate.expected_calls == 0
            || candidate.expected_calls != candidate.observed_calls
        {
            report.unsupported += 1;
            report.rejections.push((
                candidate.key.clone(),
                candidate
                    .rejected
                    .unwrap_or(if pinned_names.contains(&candidate.java_name) {
                        "unresolved_call_or_method_reference"
                    } else {
                        "incomplete_call_inventory"
                    }),
            ));
            continue;
        }
        let variable = problem.variables.len();
        problem.variables.push(Variable {
            costs: vec![
                candidate.field_copy_sites
                    * weights.0[if candidate.sync {
                        "op.arc_clone"
                    } else {
                        "op.rc_clone"
                    }],
                (1 + candidate.field_copy_sites) * weights.0["type.borrow"],
            ],
            incumbent: 0,
        });
        problem.relations.push(Relation {
            variables: vec![variable],
            rows: vec![
                (vec![0], candidate.owned_cost),
                (vec![1], candidate.borrowed_cost),
            ],
        });
        admitted.push(index);
    }
    let Ok(solution) = problem.solve(Budget::default()) else {
        report.unsupported += admitted.len();
        return report;
    };
    report.statistics = solution.statistics;
    for (variable, &index) in admitted.iter().enumerate() {
        let candidate = &candidates[index];
        let mut borrowed = solution.assignment[variable] == 1;
        if borrowed
            && plan
                .set_param_transport_shape(&candidate.key, 0, candidate.borrowed.clone())
                .is_err()
        {
            borrowed = false;
            report.unsupported += 1;
        }
        report.decisions.push(TransportDecision {
            method: candidate.key.clone(),
            parameter: 0,
            calls: candidate.expected_calls,
            old_cost: u128::from(candidate.owned_cost)
                + u128::from(problem.variables[variable].costs[0]),
            selected_cost: if borrowed {
                u128::from(candidate.borrowed_cost)
                    + u128::from(problem.variables[variable].costs[1])
            } else {
                u128::from(candidate.owned_cost) + u128::from(problem.variables[variable].costs[0])
            },
            borrowed,
        });
    }
    report
}
