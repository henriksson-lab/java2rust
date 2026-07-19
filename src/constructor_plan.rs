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
    ExternalExceptionSuperclass {
        args: &'a [Expr],
        superclass: String,
    },
    Unsupported(UnsupportedConstructorLowering),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DelegatedConstructorTrailingStmt<'a> {
    FieldAssignment {
        field_name: String,
        value: &'a Expr,
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
            if body.statements.len() == 1
                && let Some(superclass) = ty.and_then(external_exception_superclass)
            {
                return Some(ConstructorDelegationPlan::ExternalExceptionSuperclass {
                    args,
                    superclass,
                });
            }
            Some(ConstructorDelegationPlan::Unsupported(
                UnsupportedConstructorLowering::SuperRequiresInheritanceLowering,
            ))
        }
    }
}

fn external_exception_superclass(ty: &TypeDecl) -> Option<String> {
    let superclass = ty.extends.as_ref()?;
    let simple = superclass
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(superclass.as_str());
    matches!(
        simple,
        "Exception" | "RuntimeException" | "UserException" | "GATKException"
    )
    .then(|| simple.to_string())
}

fn delegated_constructor_trailing_stmts(
    stmts: &[Stmt],
) -> Option<Vec<DelegatedConstructorTrailingStmt<'_>>> {
    stmts
        .iter()
        .map(|stmt| match stmt {
            Stmt::Assign { target, value } => match target {
                Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                    Some(DelegatedConstructorTrailingStmt::FieldAssignment {
                        field_name: field.clone(),
                        value,
                    })
                }
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
            _ => None,
        })
        .collect()
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
    let inherited_fields =
        crate::inherited_members::inherited_instance_field_refs_for_type(ty, support_program);
    plan_constructor_field_values_with_inherited_field_decls(
        ty,
        body,
        signatures,
        inherited_fields
            .iter()
            .map(|inherited| inherited.field)
            .collect(),
    )
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
    plans
}

fn plan_constructor_field_value<'a>(
    field: &'a FieldDecl,
    shape: RustSlotShape,
    assigned: &[(&'a str, &'a Expr)],
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

fn constructor_field_assignments(body: Option<&Block>) -> Vec<(&str, &Expr)> {
    body.map(|body| {
        body.statements
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Assign { target, value } => match target {
                    Expr::FieldAccess(receiver, field)
                        if matches!(receiver.as_ref(), Expr::This) =>
                    {
                        Some((field.as_str(), value))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    })
    .unwrap_or_default()
}
