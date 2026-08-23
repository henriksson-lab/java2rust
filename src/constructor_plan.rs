use std::collections::HashSet;

use crate::facts::FactSet;
use crate::item_assembly::is_static_field;
use crate::java_ir::{
    Block, CallKind, ConstructorInvocationKind, Expr, Member, ResolvedCall, ResolvedConstructor,
    Stmt, TypeDecl,
};
use crate::java_ir::{FieldDecl, Program};
use crate::signature::SignaturePlan;
use crate::slot_shape::RustSlotShape;
use crate::unsupported::UnsupportedConstructorLowering;

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorDelegationPlan<'a> {
    SameClassCall {
        args: &'a [Expr],
        resolved: &'a ResolvedConstructor,
    },
    SameClassCallWithTrailing {
        args: &'a [Expr],
        resolved: &'a ResolvedConstructor,
        trailing: Vec<DelegatedConstructorTrailingStmt<'a>>,
    },
    SuperNoArgWithTrailing {
        trailing: Vec<DelegatedConstructorTrailingStmt<'a>>,
    },
    ModeledExternalSuperclassWithTrailing {
        args: &'a [Expr],
        superclass: String,
        trailing: Vec<DelegatedConstructorTrailingStmt<'a>>,
    },
    ExternalExceptionSuperclass {
        args: &'a [Expr],
        superclass: String,
    },
    ExternalExceptionSuperclassWithTrailing {
        args: &'a [Expr],
        superclass: String,
        trailing: Vec<DelegatedConstructorTrailingStmt<'a>>,
    },
    Unsupported(UnsupportedConstructorLowering),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DelegatedConstructorTrailingStmt<'a> {
    FieldAssignment {
        field_name: String,
        value: &'a Expr,
    },
    Statement {
        stmt: &'a Stmt,
    },
    ExprStatement {
        expr: &'a Expr,
    },
    InstanceCall {
        resolved: &'a ResolvedCall,
        args: &'a [Expr],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorFieldValuePlan<'a> {
    Assigned {
        field_name: String,
        value: &'a Expr,
        shape: RustSlotShape,
    },
    FieldInitializer {
        field_name: String,
        value: &'a Expr,
        shape: RustSlotShape,
    },
    Default {
        field_name: String,
        shape: RustSlotShape,
    },
}

pub fn plan_constructor_delegation(body: &Block) -> Option<ConstructorDelegationPlan<'_>> {
    plan_constructor_delegation_for_type(None, body)
}

pub fn plan_constructor_delegation_for_type<'a>(
    ty: Option<&TypeDecl>,
    body: &'a Block,
) -> Option<ConstructorDelegationPlan<'a>> {
    let Some(Stmt::ConstructorInvocation {
        kind,
        args,
        resolved,
    }) = body.statements.first()
    else {
        return None;
    };

    match kind {
        ConstructorInvocationKind::This => {
            let Some(resolved) = resolved.as_ref() else {
                return Some(ConstructorDelegationPlan::Unsupported(
                    UnsupportedConstructorLowering::UnresolvedDelegation,
                ));
            };
            if body.statements.len() == 1 {
                return Some(ConstructorDelegationPlan::SameClassCall { args, resolved });
            }
            let Some(trailing) = delegated_constructor_trailing_stmts(&body.statements[1..]) else {
                return Some(ConstructorDelegationPlan::Unsupported(
                    UnsupportedConstructorLowering::ThisWithTrailingStatements,
                ));
            };
            Some(ConstructorDelegationPlan::SameClassCallWithTrailing {
                args,
                resolved,
                trailing,
            })
        }
        ConstructorInvocationKind::Super => {
            if explicit_object_super_constructor_is_noop(ty, args, body) {
                return None;
            }
            if args.is_empty()
                && let Some(trailing) = delegated_constructor_trailing_stmts(&body.statements[1..])
            {
                return Some(ConstructorDelegationPlan::SuperNoArgWithTrailing { trailing });
            }
            if let Some(superclass) = ty.and_then(|ty| ty.extends.as_deref())
                && crate::external_packs::modeled_no_state_external_super_constructor(
                    superclass,
                    args.len(),
                )
            {
                if let Some(trailing) = delegated_constructor_trailing_stmts(&body.statements[1..])
                {
                    return Some(
                        ConstructorDelegationPlan::ModeledExternalSuperclassWithTrailing {
                            args,
                            superclass: superclass.to_string(),
                            trailing,
                        },
                    );
                }
                return Some(ConstructorDelegationPlan::Unsupported(
                    UnsupportedConstructorLowering::SuperWithUnsupportedTrailingStatements,
                ));
            }
            if let Some(superclass) = ty.and_then(external_exception_superclass) {
                if body.statements.len() == 1 {
                    return Some(ConstructorDelegationPlan::ExternalExceptionSuperclass {
                        args,
                        superclass,
                    });
                }
                if let Some(trailing) = delegated_constructor_trailing_stmts(&body.statements[1..])
                {
                    return Some(
                        ConstructorDelegationPlan::ExternalExceptionSuperclassWithTrailing {
                            args,
                            superclass,
                            trailing,
                        },
                    );
                }
            }
            Some(ConstructorDelegationPlan::Unsupported(
                UnsupportedConstructorLowering::SuperRequiresInheritanceLowering,
            ))
        }
    }
}

fn explicit_object_super_constructor_is_noop(
    ty: Option<&TypeDecl>,
    args: &[Expr],
    body: &Block,
) -> bool {
    args.is_empty()
        && type_extends_only_object(ty)
        && body
            .statements
            .iter()
            .skip(1)
            .all(is_constructor_field_assignment_stmt)
}

fn type_extends_only_object(ty: Option<&TypeDecl>) -> bool {
    let Some(superclass) = ty.and_then(|ty| ty.extends.as_deref()) else {
        return true;
    };
    matches!(
        superclass.rsplit(['.', '$']).next().unwrap_or(superclass),
        "Object"
    )
}

fn is_constructor_field_assignment_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Assign {
            target: Expr::FieldAccess(receiver, _),
            ..
        } if matches!(receiver.as_ref(), Expr::This)
    )
}

fn external_exception_superclass(ty: &TypeDecl) -> Option<String> {
    let superclass = ty.extends.as_ref()?;
    let simple = superclass
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(superclass.as_str());
    matches!(
        simple,
        "Exception"
            | "RuntimeException"
            | "IllegalArgumentException"
            | "IllegalStateException"
            | "UserException"
            | "GATKException"
    )
    .then(|| simple.to_string())
}

pub(crate) fn delegated_constructor_trailing_stmts_for_lowering(
    stmts: &[Stmt],
) -> Option<Vec<DelegatedConstructorTrailingStmt<'_>>> {
    stmts
        .iter()
        .map(|stmt| match stmt {
            Stmt::Assign { target, value } => match target {
                Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                    Some(DelegatedConstructorTrailingStmt::FieldAssignment {
                        field_name: crate::parser::to_snake_case_unescaped(field),
                        value,
                    })
                }
                Expr::Var(field) => Some(DelegatedConstructorTrailingStmt::FieldAssignment {
                    field_name: crate::parser::to_snake_case_unescaped(field),
                    value,
                }),
                _ => None,
            },
            Stmt::Expr(Expr::Call {
                receiver: None,
                args,
                resolved: Some(resolved),
                ..
            }) if resolved.kind == CallKind::Instance => {
                Some(DelegatedConstructorTrailingStmt::InstanceCall { resolved, args })
            }
            Stmt::Expr(expr) if constructor_trailing_expr_stmt_is_supported(expr) => {
                Some(DelegatedConstructorTrailingStmt::ExprStatement { expr })
            }
            Stmt::If { .. } if constructor_trailing_stmt_is_supported(stmt) => {
                Some(DelegatedConstructorTrailingStmt::Statement { stmt })
            }
            Stmt::VarDecl { .. } | Stmt::Try { .. } | Stmt::Block(_) => {
                Some(DelegatedConstructorTrailingStmt::Statement { stmt })
            }
            _ => None,
        })
        .collect()
}

fn delegated_constructor_trailing_stmts(
    stmts: &[Stmt],
) -> Option<Vec<DelegatedConstructorTrailingStmt<'_>>> {
    delegated_constructor_trailing_stmts_for_lowering(stmts)
}

fn constructor_trailing_expr_stmt_is_supported(expr: &Expr) -> bool {
    matches!(expr, Expr::Call { .. })
}

fn constructor_trailing_stmt_is_supported(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::If { .. })
}

pub fn plan_constructor_field_values<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    plan_constructor_field_values_with_inherited_field_decls(ty, body, signatures, Vec::new())
}

pub fn plan_constructor_field_values_with_inherited_fields<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
    support_program: &'a Program,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    plan_constructor_field_values_with_inherited_context(
        ty,
        body,
        signatures,
        support_program,
        &FactSet::default(),
    )
}

pub fn plan_constructor_field_values_with_inherited_context<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
    support_program: &'a Program,
    facts: &FactSet,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    let inherited_fields =
        crate::inherited_members::inherited_instance_field_refs_for_type(ty, support_program);
    let mut plans = plan_constructor_field_values_with_inherited_field_decls(
        ty,
        body,
        signatures,
        inherited_fields
            .iter()
            .map(|inherited| inherited.field)
            .collect(),
    );
    plans.extend(
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
            .into_iter()
            .map(|inherited| {
                let shape = signatures.required_field_shape(&ty.name, &inherited.rust_name);
                ConstructorFieldValuePlan::Default {
                    field_name: inherited.rust_name,
                    shape,
                }
            }),
    );
    dedup_constructor_field_value_plans(&mut plans);
    plans
}

fn plan_constructor_field_values_with_inherited_field_decls<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
    inherited_fields: Vec<&'a FieldDecl>,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    let assigned = constructor_field_assignments(body);
    let mut plans: Vec<_> = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !is_static_field(field) => {
                let shape = signatures.required_field_shape(&ty.name, &field.name);
                Some(plan_constructor_field_value(field, shape, &assigned))
            }
            _ => None,
        })
        .collect();
    plans.extend(inherited_fields.into_iter().map(|field| {
        let shape = signatures.required_field_shape(&ty.name, &field.name);
        plan_constructor_field_value(field, shape, &assigned)
    }));
    dedup_constructor_field_value_plans(&mut plans);
    plans
}

fn dedup_constructor_field_value_plans(plans: &mut Vec<ConstructorFieldValuePlan<'_>>) {
    let mut seen = HashSet::new();
    plans.retain(|plan| seen.insert(plan.field_name().to_string()));
}

impl<'a> ConstructorFieldValuePlan<'a> {
    fn field_name(&self) -> &str {
        match self {
            ConstructorFieldValuePlan::Assigned { field_name, .. }
            | ConstructorFieldValuePlan::FieldInitializer { field_name, .. }
            | ConstructorFieldValuePlan::Default { field_name, .. } => field_name,
        }
    }
}

fn plan_constructor_field_value<'a>(
    field: &'a FieldDecl,
    shape: RustSlotShape,
    assigned: &[(String, &'a Expr)],
) -> ConstructorFieldValuePlan<'a> {
    if let Some(value) = assigned
        .iter()
        .find(|(name, _)| name == &field.name)
        .map(|(_, value)| *value)
    {
        return ConstructorFieldValuePlan::Assigned {
            field_name: field.name.clone(),
            value,
            shape,
        };
    }
    if let Some(value) = field.initializer.as_ref() {
        return ConstructorFieldValuePlan::FieldInitializer {
            field_name: field.name.clone(),
            value,
            shape,
        };
    }
    ConstructorFieldValuePlan::Default {
        field_name: field.name.clone(),
        shape,
    }
}

fn constructor_field_assignments(body: Option<&Block>) -> Vec<(String, &Expr)> {
    body.map(|body| {
        body.statements
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Assign { target, value } => match target {
                    Expr::FieldAccess(receiver, field)
                        if matches!(receiver.as_ref(), Expr::This) =>
                    {
                        Some((crate::parser::to_snake_case_unescaped(field), value))
                    }
                    Expr::Var(field) => {
                        Some((crate::parser::to_snake_case_unescaped(field), value))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    })
    .unwrap_or_default()
}
