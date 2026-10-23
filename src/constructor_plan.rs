use std::collections::{HashMap, HashSet};

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

#[cfg(test)]
mod prepublication_cause_tests {
    use super::*;

    fn candidate(source: &str) -> bool {
        let parsed = crate::parser::parse_java_source(source);
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let ty = &parsed.program.types[0];
        let ctor = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Constructor(ctor) => Some(ctor),
                _ => None,
            })
            .unwrap();
        let body = ctor.body.as_ref().unwrap();
        let trailing = body
            .statements
            .iter()
            .skip(1)
            .map(|stmt| DelegatedConstructorTrailingStmt::Statement { stmt })
            .collect::<Vec<_>>();
        prepublication_cause_initialization_candidate(ty, ctor, body, &trailing).is_some()
    }

    #[test]
    fn prepublication_cause_candidate_accepts_only_a_discarded_immediate_parameter_or_null_call() {
        for call in [
            "initCause(cause)",
            "this.initCause(cause)",
            "super.initCause(cause)",
            "initCause(null)",
        ] {
            assert!(
                candidate(&format!(
                    "class Wrap extends RuntimeException {{ Wrap(String msg, Throwable cause) {{ super(msg); {call}; }} }}"
                )),
                "{call}"
            );
        }
        assert!(candidate(
            "class Wrap extends RuntimeException { Wrap(Throwable cause) { super(); initCause(cause); } }"
        ));
    }

    #[test]
    fn prepublication_cause_candidate_rejects_escape_initializer_dispatch_and_reinitialization() {
        for source in [
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg, cause); initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { this(msg); initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); initCause(cause); initCause(null); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); observe(this); initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); Object result = initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause, Throwable other) { super(msg); other.initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); initCause(this); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); initCause(makeCause()); } }",
            "class Wrap extends RuntimeException { Object state = publish(this); Wrap(String msg, Throwable cause) { super(msg); initCause(cause); } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); initCause(cause); } public Throwable initCause(Throwable cause) { return this; } }",
            "class Wrap extends RuntimeException { Wrap(String msg, Throwable cause) { super(msg); initCause(cause); } public Throwable fillInStackTrace() { return this; } }",
        ] {
            assert!(!candidate(source), "{source}");
        }
    }
}

/// Ordered declaration inventory shared by enum metadata and initialization.
/// Self-typed fields and static aliases are deliberately excluded.
pub(crate) fn enum_requires_instance_initialization(ty: &TypeDecl) -> bool {
    ty.kind == crate::java_ir::TypeKind::Enum
        && ty.members.iter().any(|member| match member {
            Member::Constructor(_) => true,
            Member::Field(field) => {
                !field.modifiers.contains(&crate::java_ir::Modifier::Static)
                    || (field.is_enum_constant()
                        && matches!(field.initializer, Some(Expr::New { body: Some(_), .. })))
            }
            _ => false,
        })
}

pub fn enum_constant_fields(ty: &TypeDecl) -> impl Iterator<Item = &FieldDecl> {
    ty.members.iter().filter_map(|member| match member {
        Member::Field(field)
            if ty.kind == crate::java_ir::TypeKind::Enum && field.is_enum_constant() =>
        {
            Some(field)
        }
        _ => None,
    })
}

#[derive(Debug)]
pub enum EnumConstructorSelection<'a> {
    Resolved {
        declaration: &'a crate::java_ir::ConstructorDecl,
        call: &'a ResolvedConstructor,
    },
    ImplicitDefault,
    /// Preserve an unresolved boundary; never choose the first/arity-only ctor.
    Unresolved,
}

#[derive(Debug)]
pub struct EnumConstantInitialization<'a> {
    pub field: &'a FieldDecl,
    pub ordinal: usize,
    pub member_index: usize,
    pub arguments: &'a [Expr],
    pub body: Option<&'a [Member]>,
    pub constructor: EnumConstructorSelection<'a>,
}

/// No evaluation or initializer substitution happens here. The source AST and
/// resolved constructor ABI remain authoritative for the eventual storage,
/// construction and publication steps, including zero-argument constants.
#[derive(Debug)]
pub struct EnumInitializationPlan<'a> {
    pub owner: &'a TypeDecl,
    pub constants: Vec<EnumConstantInitialization<'a>>,
    pub instance_fields: Vec<&'a FieldDecl>,
}

impl<'a> EnumInitializationPlan<'a> {
    pub fn for_type(owner: &'a TypeDecl) -> Option<Self> {
        if owner.kind != crate::java_ir::TypeKind::Enum {
            return None;
        }
        let mut constructors = HashMap::new();
        for member in &owner.members {
            if let Member::Constructor(constructor) = member {
                let key = crate::signature::ConstructorKey::source_constructor(owner, constructor);
                constructors
                    .entry(key)
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(constructor));
            }
        }
        let constants = owner
            .members
            .iter()
            .enumerate()
            .filter_map(|(member_index, member)| match member {
                Member::Field(field) if field.is_enum_constant() => Some((member_index, field)),
                _ => None,
            })
            .enumerate()
            .map(|(ordinal, (member_index, field))| {
                let (arguments, body, constructor) = match field.initializer.as_ref() {
                    Some(Expr::New {
                        args,
                        body,
                        resolved,
                        ..
                    }) => {
                        let constructor = if let Some(call) = resolved {
                            let key = crate::signature::ConstructorKey::resolved_constructor(call);
                            constructors.get(&key).copied().flatten().map_or(
                                EnumConstructorSelection::Unresolved,
                                |declaration| EnumConstructorSelection::Resolved {
                                    declaration,
                                    call,
                                },
                            )
                        } else if constructors.is_empty() && args.is_empty() {
                            EnumConstructorSelection::ImplicitDefault
                        } else {
                            EnumConstructorSelection::Unresolved
                        };
                        (args.as_slice(), body.as_deref(), constructor)
                    }
                    _ => (&[][..], None, EnumConstructorSelection::Unresolved),
                };
                EnumConstantInitialization {
                    field,
                    ordinal,
                    member_index,
                    arguments,
                    body,
                    constructor,
                }
            })
            .collect();
        let instance_fields = owner
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Field(field)
                    if !field.modifiers.contains(&crate::java_ir::Modifier::Static) =>
                {
                    Some(field)
                }
                _ => None,
            })
            .collect();
        Some(Self {
            owner,
            constants,
            instance_fields,
        })
    }
}

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
    ExternalSuperFieldAssignment {
        field_name: String,
        value: &'a Expr,
    },
    Statement {
        stmt: &'a Stmt,
    },
    OwnedStatement {
        stmt: Stmt,
        /// Lexical class whose constructor body supplied this copied
        /// statement. Calls to private methods and `super` remain statically
        /// bound to that class after constructor flattening.
        lexical_declaring_type: Option<&'a TypeDecl>,
        /// Instance initializers run in class scope, outside constructor
        /// parameter/local bindings, even when replayed inside its Rust fn.
        initializer_scope: bool,
    },
    ExprStatement {
        expr: &'a Expr,
    },
    InstanceCall {
        resolved: &'a ResolvedCall,
        args: &'a [Expr],
    },
}

/// Structural proof for an atomic, prepublication cause initialization. The
/// caller must additionally prove a native exception base, a cause-uninitialized
/// superclass overload, and a Throwable/null cause domain. This is not a plan
/// for arbitrary Throwable mutation after an object has escaped.
pub(crate) fn prepublication_cause_initialization_candidate<'a>(
    ty: &TypeDecl,
    ctor: &crate::java_ir::ConstructorDecl,
    body: &'a Block,
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
) -> Option<(&'a [Expr], &'a Expr)> {
    if ty.members.iter().any(|member| match member {
        Member::Field(field) => !is_static_field(field) && field.initializer.is_some(),
        Member::Method(method) => {
            (matches!(method.name.as_str(), "initCause" | "init_cause") && method.params.len() == 1)
                || (matches!(
                    method.name.as_str(),
                    "fillInStackTrace" | "fill_in_stack_trace"
                ) && method.params.is_empty())
        }
        _ => false,
    }) {
        return None;
    }
    let [
        Stmt::ConstructorInvocation {
            kind: ConstructorInvocationKind::Super,
            args: super_args,
            ..
        },
        Stmt::Expr(
            call @ Expr::Call {
                receiver,
                name,
                args,
                resolved,
                ..
            },
        ),
    ] = body.statements.as_slice()
    else {
        return None;
    };
    if super_args.len() > 1
        || !matches!(name.as_str(), "initCause" | "init_cause")
        || !receiver
            .as_deref()
            .is_none_or(|receiver| matches!(receiver, Expr::This | Expr::Super))
    {
        return None;
    }
    let [cause] = args.as_slice() else {
        return None;
    };
    if !matches!(cause, Expr::Literal(crate::java_ir::Literal::Null))
        && !matches!(cause, Expr::Var(name) if ctor.params.iter().any(|param| {
            param.name == *name || param.source_name == *name
        }))
    {
        return None;
    }
    // Subsuming a source call is legal only if it is exactly the expression
    // the normal delegation planner would replay, not another trailing effect.
    let [trailing] = trailing else {
        return None;
    };
    let same_call = match trailing {
        DelegatedConstructorTrailingStmt::ExprStatement { expr } => *expr == call,
        DelegatedConstructorTrailingStmt::Statement { stmt } => {
            matches!(stmt, Stmt::Expr(expr) if expr == call)
        }
        DelegatedConstructorTrailingStmt::OwnedStatement { stmt, .. } => {
            matches!(stmt, Stmt::Expr(expr) if expr == call)
        }
        DelegatedConstructorTrailingStmt::InstanceCall {
            resolved: planned,
            args: planned_args,
        } => resolved.as_ref() == Some(*planned) && *planned_args == args.as_slice(),
        _ => false,
    };
    same_call.then_some((super_args, cause))
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

/// Field initializers are an execution phase, not allocation-time defaults.
/// Keep them in declaration order and retain the lexical owner so inherited
/// field hiding and private calls use the same routing as constructor bodies.
pub(crate) fn instance_field_initializer_statements(
    ty: &TypeDecl,
) -> Vec<DelegatedConstructorTrailingStmt<'_>> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            if is_static_field(field) {
                return None;
            }
            Some(instance_field_initializer_statement(
                ty,
                &field.name,
                field.initializer.as_ref()?,
            ))
        })
        .collect()
}

pub(crate) fn instance_field_initializer_statement<'a>(
    ty: &'a TypeDecl,
    field_name: &str,
    value: &Expr,
) -> DelegatedConstructorTrailingStmt<'a> {
    DelegatedConstructorTrailingStmt::OwnedStatement {
        stmt: Stmt::Assign {
            target: Expr::FieldAccess(Box::new(Expr::This), field_name.to_string()),
            value: value.clone(),
        },
        lexical_declaring_type: Some(ty),
        initializer_scope: true,
    }
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
    crate::stdlib::is_known_throwable_carrier_type(superclass).then(|| superclass.clone())
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
                // A bare Java name is not structurally a field access: a local or
                // parameter with the same name shadows the field. Keep it as a
                // normal statement so lowering can use the binding-aware shape
                // environment to distinguish the cases. Explicit `this.field`
                // assignments remain on the constructor-field fast path above.
                Expr::Var(_) => Some(DelegatedConstructorTrailingStmt::Statement { stmt }),
                _ => Some(DelegatedConstructorTrailingStmt::Statement { stmt }),
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
            // Once construction has established `__java_self`, every
            // remaining Java statement is an ordinary body statement. Keep
            // the specialized cases above for field initialization and
            // implicit-this calls, and route all other control flow through
            // the shared statement lowerer instead of silently discarding an
            // entire constructor when it contains a loop, switch, return, or
            // other previously unlisted statement.
            _ => Some(DelegatedConstructorTrailingStmt::Statement { stmt }),
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

pub fn plan_constructor_field_values<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    plan_constructor_field_values_with_inherited_field_decls(ty, body, signatures, Vec::new())
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
    plan_constructor_field_values_with_inherited_field_refs(
        ty,
        body,
        signatures,
        inherited_fields,
        facts,
    )
}

pub(crate) fn plan_constructor_field_values_with_inherited_field_refs<'a>(
    ty: &'a TypeDecl,
    body: Option<&'a Block>,
    signatures: &SignaturePlan,
    inherited_fields: Vec<crate::inherited_members::InheritedInstanceFieldRef<'a>>,
    facts: &FactSet,
) -> Vec<ConstructorFieldValuePlan<'a>> {
    let mut plans =
        plan_constructor_field_values_with_inherited_field_decls(ty, body, signatures, Vec::new());
    let assigned = constructor_field_assignments(body);
    plans.extend(inherited_fields.iter().map(|inherited| {
        let field = inherited.field();
        let base_name =
            crate::java_naming::allocate_rust_ident(&field.name, &std::collections::HashSet::new());
        let rust_name = crate::inherited_members::inherited_instance_field_rust_name(
            ty,
            inherited,
            &inherited_fields,
            &base_name,
        );
        let shape = signatures.required_field_shape(&ty.name, &rust_name);
        let visible_assignments = if rust_name == base_name {
            assigned.as_slice()
        } else {
            &[]
        };
        plan_constructor_field_value(field, shape, visible_assignments).with_field_name(rust_name)
    }));
    plans.extend(
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
            .into_iter()
            .map(|inherited| {
                let shape = signatures.required_field_shape(&ty.name, inherited.rust_name());
                ConstructorFieldValuePlan::Default {
                    field_name: inherited.rust_name().to_string(),
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
                let shape = signatures.required_declared_field_shape(ty, field);
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
    pub(crate) fn specialize_type_variables(
        &mut self,
        substitutions: &std::collections::HashMap<String, crate::java_ir::JavaType>,
    ) {
        let shape = match self {
            Self::Assigned { shape, .. }
            | Self::FieldInitializer { shape, .. }
            | Self::Default { shape, .. } => shape,
        };
        let specialized =
            crate::resolve::substitute_method_type_variables(&shape.java_ty, substitutions);
        *shape = crate::shape_projection::specialize_slot_shape_java_type(shape, &specialized);
    }

    fn field_name(&self) -> &str {
        match self {
            ConstructorFieldValuePlan::Assigned { field_name, .. }
            | ConstructorFieldValuePlan::FieldInitializer { field_name, .. }
            | ConstructorFieldValuePlan::Default { field_name, .. } => field_name,
        }
    }

    fn with_field_name(self, field_name: String) -> Self {
        match self {
            ConstructorFieldValuePlan::Assigned { value, shape, .. } => {
                ConstructorFieldValuePlan::Assigned {
                    field_name,
                    value,
                    shape,
                }
            }
            ConstructorFieldValuePlan::FieldInitializer { value, shape, .. } => {
                ConstructorFieldValuePlan::FieldInitializer {
                    field_name,
                    value,
                    shape,
                }
            }
            ConstructorFieldValuePlan::Default { shape, .. } => {
                ConstructorFieldValuePlan::Default { field_name, shape }
            }
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
                    // A bare name can be a parameter or local that shadows a
                    // field. It is deliberately retained as an ordered
                    // statement by `delegated_constructor_trailing_stmts_for_lowering`,
                    // where the binding-aware environment can distinguish the
                    // cases. Claiming it here as a field assignment lowers the
                    // same RHS twice (and can move `this` on the first copy).
                    Expr::Var(_) => None,
                    _ => None,
                },
                _ => None,
            })
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn enum_initialization_retains_actual_declarations_arguments_and_overload_selection() {
        use super::*;
        let parsed = crate::parser::parse_java_source(
            r#"
enum E {
    ONE(1), ALIAS(ONE), EMPTY, SPECIAL(3) { int value() { return 4; } };
    int stored;
    E peer;
    static final E SELECTED = ONE;
    E(int value) { stored = value; }
    E(E alias) { stored = alias.stored; }
    E() { stored = 0; }
    int value() { return stored; }
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let unresolved = EnumInitializationPlan::for_type(&parsed.program.types[0]).unwrap();
        assert_eq!(unresolved.constants.len(), 4);
        assert!(
            unresolved.constants.iter().all(|constant| matches!(
                constant.constructor,
                EnumConstructorSelection::Unresolved
            ))
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let plan = EnumInitializationPlan::for_type(&resolved.program.types[0]).unwrap();
        assert_eq!(
            plan.instance_fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["stored", "peer"]
        );
        assert_eq!(
            plan.constants
                .iter()
                .map(|constant| constant.field.name.as_str())
                .collect::<Vec<_>>(),
            ["ONE", "ALIAS", "EMPTY", "SPECIAL"]
        );
        for (ordinal, constant) in plan.constants.iter().enumerate() {
            assert_eq!(constant.ordinal, ordinal);
            assert_eq!(constant.member_index, ordinal);
            let EnumConstructorSelection::Resolved { declaration, call } = &constant.constructor
            else {
                panic!("unresolved constant: {constant:?}");
            };
            assert_eq!(declaration.params.len(), constant.arguments.len());
            assert_eq!(
                crate::signature::ConstructorKey::source_constructor(plan.owner, declaration),
                crate::signature::ConstructorKey::resolved_constructor(call)
            );
            match ordinal {
                0 | 3 => assert_eq!(
                    declaration.params[0].ty,
                    crate::parser::parse_hierarchy_type("int")
                ),
                1 => assert_eq!(
                    declaration.params[0].ty,
                    crate::parser::parse_hierarchy_type("E")
                ),
                2 => assert!(constant.arguments.is_empty()),
                _ => unreachable!(),
            }
        }
        assert!(plan.constants[3].body.is_some());
        assert!(
            plan.constants[..3]
                .iter()
                .all(|constant| constant.body.is_none())
        );
        assert_eq!(
            crate::object_model::java_enum_variant_names(plan.owner),
            ["ONE", "ALIAS", "EMPTY", "SPECIAL"]
        );
    }

    #[test]
    fn zero_argument_enum_constants_retain_implicit_construction_sites() {
        use super::*;
        let parsed = crate::parser::parse_java_source(
            "enum E { FIRST, SECOND; E link; static E selected = FIRST; }",
        );
        let owner = &parsed.program.types[0];
        let plan = EnumInitializationPlan::for_type(owner).unwrap();
        assert_eq!(plan.constants.len(), 2);
        for constant in &plan.constants {
            assert!(matches!(constant.field.initializer, Some(Expr::New { .. })));
            assert!(constant.arguments.is_empty());
            assert!(matches!(
                constant.constructor,
                EnumConstructorSelection::ImplicitDefault
            ));
        }
        assert_eq!(
            crate::object_model::java_enum_variant_names(owner),
            ["FIRST", "SECOND"]
        );
        assert_eq!(plan.instance_fields.len(), 1);
        assert!(
            EnumInitializationPlan::for_type(
                &crate::parser::parse_java_source("class C {}").program.types[0]
            )
            .is_none()
        );
    }

    use super::*;
    use crate::slot_shape::{DispatchShape, OwnershipShape, ViewShape};

    #[test]
    fn field_plan_specialization_is_simultaneous_and_preserves_slot_metadata() {
        let mut shape =
            RustSlotShape::for_storage(crate::parser::parse_hierarchy_type("Pair<Optional<A>, B>"));
        shape.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Pair".into(),
        };
        shape.view = ViewShape::DynTraitView;
        shape.ownership = OwnershipShape::SharedBorrow;
        let mut expected = shape.clone();
        expected.java_ty = crate::parser::parse_hierarchy_type("Pair<Optional<B>, String>");
        let substitutions = std::collections::HashMap::from([
            ("A".into(), crate::parser::parse_hierarchy_type("B")),
            ("B".into(), crate::java_ir::JavaType::String),
        ]);
        let value = Expr::Var("argument".into());
        let mut plans = [
            ConstructorFieldValuePlan::Assigned {
                field_name: "item".into(),
                value: &value,
                shape: shape.clone(),
            },
            ConstructorFieldValuePlan::FieldInitializer {
                field_name: "item".into(),
                value: &value,
                shape: shape.clone(),
            },
            ConstructorFieldValuePlan::Default {
                field_name: "item".into(),
                shape,
            },
        ];
        for plan in &mut plans {
            plan.specialize_type_variables(&substitutions);
            assert_eq!(plan.field_name(), "item");
            let actual = match plan {
                ConstructorFieldValuePlan::Assigned {
                    shape,
                    value: actual,
                    ..
                }
                | ConstructorFieldValuePlan::FieldInitializer {
                    shape,
                    value: actual,
                    ..
                } => {
                    assert_eq!(*actual, &value);
                    shape
                }
                ConstructorFieldValuePlan::Default { shape, .. } => shape,
            };
            assert_eq!(actual, &expected);
        }
    }
}
