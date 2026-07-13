use crate::java_ir::{
    Block, ConstructorInvocationKind, Expr, Member, ResolvedConstructor, Stmt, TypeDecl,
};
use crate::signature::SignaturePlan;
use crate::slot_shape::RustSlotShape;
use crate::unsupported::UnsupportedConstructorLowering;

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorDelegationPlan<'a> {
    SameClassCall {
        args: &'a [Expr],
        resolved: &'a ResolvedConstructor,
    },
    Unsupported(UnsupportedConstructorLowering),
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
    let Some(Stmt::ConstructorInvocation {
        kind,
        args,
        resolved,
    }) = body.statements.first()
    else {
        return None;
    };

    match kind {
        ConstructorInvocationKind::This if body.statements.len() != 1 => {
            Some(ConstructorDelegationPlan::Unsupported(
                UnsupportedConstructorLowering::ThisWithTrailingStatements,
            ))
        }
        ConstructorInvocationKind::This => resolved
            .as_ref()
            .map(|resolved| ConstructorDelegationPlan::SameClassCall { args, resolved })
            .or({
                Some(ConstructorDelegationPlan::Unsupported(
                    UnsupportedConstructorLowering::UnresolvedDelegation,
                ))
            }),
        ConstructorInvocationKind::Super => Some(ConstructorDelegationPlan::Unsupported(
            UnsupportedConstructorLowering::SuperRequiresInheritanceLowering,
        )),
    }
}

pub fn plan_constructor_field_values<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    let assigned = constructor_field_assignments(body);
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => {
                let shape = signatures.required_field_shape(&ty.name, &field.name);
                if let Some(value) = assigned
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| *value)
                {
                    return Some(ConstructorFieldValuePlan::Assigned {
                        field_name: field.name.clone(),
                        value,
                        shape,
                    });
                }
                if let Some(value) = field.initializer.as_ref() {
                    return Some(ConstructorFieldValuePlan::FieldInitializer {
                        field_name: field.name.clone(),
                        value,
                        shape,
                    });
                }
                Some(ConstructorFieldValuePlan::Default {
                    field_name: field.name.clone(),
                    shape,
                })
            }
            _ => None,
        })
        .collect()
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
