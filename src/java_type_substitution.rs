//! Binder-aware specialization of source members, including their bodies.
//! Binding IDs and declaration locations are retained; only type occurrences
//! change. A member/local class's own type parameters shadow enclosing ones.
use crate::java_ir::*;
use std::{borrow::Cow, collections::HashMap};

type Substitutions = HashMap<String, JavaType>;

pub(crate) fn rename_class_parameter(declaration: &mut TypeDecl, index: usize, new_name: String) {
    let old = std::mem::replace(&mut declaration.type_params[index], new_name.clone());
    declaration_contents(
        declaration,
        &HashMap::from([(
            old,
            JavaType::Named {
                name: new_name,
                args: vec![],
            },
        )]),
    );
}

pub(crate) fn rename_method_parameter(method: &mut MethodDecl, index: usize, new_name: String) {
    let old = std::mem::replace(&mut method.type_params[index], new_name.clone());
    method_types(
        method,
        &HashMap::from([(
            old,
            JavaType::Named {
                name: new_name,
                args: vec![],
            },
        )]),
    );
}

pub(crate) fn rename_constructor_parameter(
    constructor: &mut ConstructorDecl,
    index: usize,
    new_name: String,
) {
    let old = std::mem::replace(&mut constructor.type_params[index], new_name.clone());
    constructor_types(
        constructor,
        &HashMap::from([(
            old,
            JavaType::Named {
                name: new_name,
                args: vec![],
            },
        )]),
    );
}

pub(crate) fn specialized_method(method: &MethodDecl, substitutions: &Substitutions) -> MethodDecl {
    let mut result = method.clone();
    method_types(&mut result, substitutions);
    result
}

pub(crate) fn specialized_locals(
    method: &MethodDecl,
    locals: &[crate::signature::LocalPlan],
    substitutions: &Substitutions,
) -> Vec<crate::signature::LocalPlan> {
    let substitutions = without_shadowed_binders(substitutions, &method.type_params);
    locals
        .iter()
        .cloned()
        .map(|mut local| {
            let java_ty =
                crate::resolve::substitute_method_type_variables(local.java_ty(), &substitutions);
            local.set_java_ty(java_ty);
            ty(&mut local.shape_mut().java_ty, &substitutions);
            local
        })
        .collect()
}

pub(crate) fn without_shadowed_binders<'a>(
    substitutions: &'a Substitutions,
    binders: &[String],
) -> Cow<'a, Substitutions> {
    if binders.is_empty() || !substitutions.keys().any(|name| binders.contains(name)) {
        return Cow::Borrowed(substitutions);
    }
    Cow::Owned(
        substitutions
            .iter()
            .filter(|(name, _)| !binders.contains(name))
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect(),
    )
}

fn ty(ty: &mut JavaType, substitutions: &Substitutions) {
    *ty = crate::resolve::substitute_method_type_variables(ty, substitutions);
}

fn method_types(method: &mut MethodDecl, substitutions: &Substitutions) {
    let substitutions = without_shadowed_binders(substitutions, &method.type_params);
    ty(&mut method.return_type, &substitutions);
    for param in &mut method.params {
        ty(&mut param.ty, &substitutions);
    }
    for bound in method
        .type_param_bounds
        .iter_mut()
        .flatten()
        .chain(method.throws.iter_mut())
    {
        ty(bound, &substitutions);
    }
    if let Some(body) = &mut method.body {
        block(body, &substitutions);
    }
}

fn member(member: &mut Member, substitutions: &Substitutions) {
    match member {
        Member::Method(method) => method_types(method, substitutions),
        Member::Field(field) => {
            ty(&mut field.ty, substitutions);
            if let Some(value) = &mut field.initializer {
                expr(value, substitutions);
            }
        }
        Member::Constructor(constructor) => constructor_types(constructor, substitutions),
        Member::Type(declaration) => declaration_types(declaration, substitutions),
        Member::Unsupported { .. } => {}
    }
}

fn constructor_types(constructor: &mut ConstructorDecl, substitutions: &Substitutions) {
    let substitutions = without_shadowed_binders(substitutions, &constructor.type_params);
    for param in &mut constructor.params {
        ty(&mut param.ty, &substitutions);
    }
    for bound in constructor
        .type_param_bounds
        .iter_mut()
        .flatten()
        .chain(constructor.throws.iter_mut())
    {
        ty(bound, &substitutions);
    }
    if let Some(body) = &mut constructor.body {
        block(body, &substitutions);
    }
}

fn declaration_types(declaration: &mut TypeDecl, substitutions: &Substitutions) {
    let mut binders = declaration.type_params.clone();
    binders.push(
        declaration
            .name
            .rsplit('.')
            .next()
            .unwrap_or(&declaration.name)
            .to_string(),
    );
    let substitutions = without_shadowed_binders(substitutions, &binders);
    declaration_contents(declaration, &substitutions);
}

fn declaration_contents(declaration: &mut TypeDecl, substitutions: &Substitutions) {
    for bound in declaration.type_param_bounds.iter_mut().flatten() {
        ty(bound, &substitutions);
    }
    for name in declaration
        .extends
        .iter_mut()
        .chain(declaration.implements.iter_mut())
    {
        let specialized = crate::resolve::substitute_method_type_variables(
            &crate::parser::parse_hierarchy_type(name),
            &substitutions,
        );
        *name = java_type_text(&specialized);
    }
    for item in &mut declaration.members {
        member(item, &substitutions);
    }
    for init in &mut declaration.static_init_blocks {
        block(&mut init.block, &substitutions);
    }
}

fn block(body: &mut Block, substitutions: &Substitutions) {
    let mut scope = Cow::Borrowed(substitutions);
    for statement in &mut body.statements {
        if let Stmt::LocalType(declaration) = statement {
            let name = declaration
                .name
                .rsplit('.')
                .next()
                .unwrap_or(&declaration.name);
            if scope.contains_key(name) {
                scope.to_mut().remove(name);
            }
        }
        stmt(statement, &scope);
    }
}

fn stmt(statement: &mut Stmt, substitutions: &Substitutions) {
    match statement {
        Stmt::VarDecl {
            ty: declared,
            initializer,
            ..
        } => {
            ty(declared, substitutions);
            if let Some(value) = initializer {
                expr(value, substitutions);
            }
        }
        Stmt::Assign { target, value } => {
            expr(target, substitutions);
            expr(value, substitutions);
        }
        Stmt::Expr(value) | Stmt::Return(Some(value)) | Stmt::Throw(value) => {
            expr(value, substitutions)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr(condition, substitutions);
            block(then_block, substitutions);
            if let Some(body) = else_block {
                block(body, substitutions);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr(condition, substitutions);
            block(body, substitutions);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for statement in init {
                stmt(statement, substitutions);
            }
            if let Some(value) = condition {
                expr(value, substitutions);
            }
            for value in update {
                expr(value, substitutions);
            }
            block(body, substitutions);
        }
        Stmt::EnhancedFor {
            var_type,
            iterable,
            body,
            ..
        } => {
            ty(var_type, substitutions);
            expr(iterable, substitutions);
            block(body, substitutions);
        }
        Stmt::Switch { selector, cases } => {
            expr(selector, substitutions);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(value) = label {
                        expr(value, substitutions);
                    }
                }
                for statement in &mut case.statements {
                    stmt(statement, substitutions);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for statement in resources {
                stmt(statement, substitutions);
            }
            block(body, substitutions);
            for catch in catches {
                for caught in &mut catch.types {
                    ty(caught, substitutions);
                }
                block(&mut catch.body, substitutions);
            }
            if let Some(body) = finally_block {
                block(body, substitutions);
            }
        }
        Stmt::LocalType(declaration) => declaration_types(declaration, substitutions),
        Stmt::ConstructorInvocation { args, resolved, .. } => {
            for value in args {
                expr(value, substitutions);
            }
            if let Some(call) = resolved {
                constructor(call, substitutions);
            }
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => block(body, substitutions),
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn constructor(call: &mut ResolvedConstructor, substitutions: &Substitutions) {
    ty(&mut call.constructed_type, substitutions);
    for param in &mut call.params {
        ty(&mut param.ty, substitutions);
    }
    for coercion in &mut call.arg_coercions {
        ty(&mut coercion.actual, substitutions);
        ty(&mut coercion.target, substitutions);
    }
}

fn expr(value: &mut Expr, substitutions: &Substitutions) {
    match value {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr(receiver, substitutions)
        }
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            expr(left, substitutions);
            expr(right, substitutions);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            ty(element_type, substitutions);
            for value in dimensions {
                expr(value, substitutions);
            }
            if let Some(values) = initializer {
                for value in values {
                    expr(value, substitutions);
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for value in values {
                expr(value, substitutions);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr(condition, substitutions);
            expr(then_expr, substitutions);
            expr(else_expr, substitutions);
        }
        Expr::Switch { selector, cases } => {
            expr(selector, substitutions);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(value) = label {
                        expr(value, substitutions);
                    }
                }
                expr(&mut case.value, substitutions);
            }
        }
        Expr::Call {
            receiver,
            args,
            resolved,
            ..
        } => {
            if let Some(receiver) = receiver {
                expr(receiver, substitutions);
            }
            for value in args {
                expr(value, substitutions);
            }
            if let Some(call) = resolved {
                ty(&mut call.return_type, substitutions);
                for target in &mut call.arg_target_types {
                    ty(target, substitutions);
                }
                for coercion in &mut call.arg_coercions {
                    ty(&mut coercion.actual, substitutions);
                    ty(&mut coercion.target, substitutions);
                }
            }
        }
        Expr::New {
            ty: declared,
            qualifier,
            args,
            body,
            resolved,
            ..
        } => {
            ty(declared, substitutions);
            if let Some(value) = qualifier {
                expr(value, substitutions);
            }
            for value in args {
                expr(value, substitutions);
            }
            if let Some(body) = body {
                for item in body {
                    member(item, substitutions);
                }
            }
            if let Some(call) = resolved {
                constructor(call, substitutions);
            }
        }
        Expr::Cast {
            ty: declared,
            expr: value,
        }
        | Expr::InstanceOf {
            ty: declared,
            expr: value,
            ..
        } => {
            ty(declared, substitutions);
            expr(value, substitutions);
        }
        Expr::Lambda { params, body } => {
            for param in params {
                if let Some(declared) = &mut param.ty {
                    ty(declared, substitutions);
                }
            }
            match body {
                LambdaBody::Expr(value) => expr(value, substitutions),
                LambdaBody::Block(body) => block(body, substitutions),
            }
        }
        Expr::MethodReference { target, .. } => {
            if let Some(declared) = substitutions.get(target) {
                *target = java_type_text(declared);
            }
        }
        Expr::ClassLiteral(declared) => ty(declared, substitutions),
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

pub(crate) fn java_type_text(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::String => "java.lang.String".to_string(),
        JavaType::Primitive(primitive) => match primitive {
            PrimitiveType::Boolean => "boolean",
            PrimitiveType::Byte => "byte",
            PrimitiveType::Short => "short",
            PrimitiveType::Char => "char",
            PrimitiveType::Int => "int",
            PrimitiveType::Long => "long",
            PrimitiveType::Float => "float",
            PrimitiveType::Double => "double",
        }
        .to_string(),
        JavaType::Named { name, args } if !args.is_empty() => format!(
            "{name}<{}>",
            args.iter()
                .map(java_type_text)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        JavaType::Array(inner) => format!("{}[]", java_type_text(inner)),
        JavaType::Intersection(bounds) => bounds
            .iter()
            .map(java_type_text)
            .collect::<Vec<_>>()
            .join(" & "),
        JavaType::Wildcard(None) => "?".to_string(),
        JavaType::Wildcard(Some(WildcardBound::Extends(inner))) => {
            format!("? extends {}", java_type_text(inner))
        }
        JavaType::Wildcard(Some(WildcardBound::Super(inner))) => {
            format!("? super {}", java_type_text(inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_parameter_renaming_respects_callable_binders_and_qualified_types() {
        let mut parsed = crate::parser::parse_java_source(
            "class Owner<Integer> { Integer value; java.lang.Integer literal; <Integer> Integer own(Integer value) { return value; } Integer outer(Integer value) { return value; } }",
        );
        let owner = &mut parsed.program.types[0];
        rename_class_parameter(owner, 0, "Payload".into());
        assert_eq!(owner.type_params, ["Payload"]);
        let Member::Field(value) = &owner.members[0] else {
            panic!()
        };
        assert!(matches!(&value.ty, JavaType::Named { name, .. } if name == "Payload"));
        let Member::Field(literal) = &owner.members[1] else {
            panic!()
        };
        assert!(matches!(&literal.ty, JavaType::Named { name, .. } if name == "java.lang.Integer"));
        let Member::Method(own) = &owner.members[2] else {
            panic!()
        };
        assert_eq!(own.type_params, ["Integer"]);
        assert!(matches!(&own.return_type, JavaType::Named { name, .. } if name == "Integer"));
        let Member::Method(outer) = &owner.members[3] else {
            panic!()
        };
        assert!(matches!(&outer.return_type, JavaType::Named { name, .. } if name == "Payload"));
        assert!(matches!(&outer.params[0].ty, JavaType::Named { name, .. } if name == "Payload"));
    }

    #[test]
    fn binder_filter_borrows_unmodified_maps_and_removes_only_shadowed_entries() {
        let substitutions = HashMap::from([
            ("T".to_string(), JavaType::String),
            ("U".to_string(), JavaType::Primitive(PrimitiveType::Int)),
        ]);
        assert!(matches!(
            without_shadowed_binders(&substitutions, &[]),
            Cow::Borrowed(_)
        ));
        assert!(matches!(
            without_shadowed_binders(&substitutions, &["Other".to_string()]),
            Cow::Borrowed(_)
        ));
        let filtered = without_shadowed_binders(&substitutions, &["T".to_string()]);
        assert!(matches!(filtered, Cow::Owned(_)));
        assert!(!filtered.contains_key("T"));
        assert_eq!(filtered.get("U"), substitutions.get("U"));
        assert_eq!(substitutions.len(), 2);
    }

    #[test]
    fn body_types_specialize_without_renaming_variables_or_shadowed_binders() {
        let parsed = crate::parser::parse_java_source(
            "class Owner<T> { T copy(T value) { T local = value; class Nested<T> { T item; } return local; } <T> T identity(T value) { T local = value; return local; } }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let Member::Method(method) = &resolved.program.types[0].members[0] else {
            panic!()
        };
        let substitutions = HashMap::from([("T".to_string(), JavaType::String)]);
        let specialized = specialized_method(method, &substitutions);
        assert_eq!(specialized.return_type, JavaType::String);
        assert_eq!(specialized.params[0].binding, method.params[0].binding);
        let Stmt::VarDecl {
            ty: local,
            initializer,
            binding,
            ..
        } = &specialized.body.as_ref().unwrap().statements[0]
        else {
            panic!()
        };
        assert_eq!(*local, JavaType::String);
        assert!(matches!(initializer, Some(Expr::Var(name)) if name == "value"));
        let Stmt::VarDecl {
            binding: original_binding,
            ..
        } = &method.body.as_ref().unwrap().statements[0]
        else {
            panic!()
        };
        assert_eq!(binding, original_binding);
        assert!(binding.is_some());
        let Stmt::LocalType(nested) = &specialized.body.as_ref().unwrap().statements[1] else {
            panic!()
        };
        let Member::Field(field) = &nested.members[0] else {
            panic!()
        };
        assert!(matches!(&field.ty, JavaType::Named { name, .. } if name == "T"));
        let Member::Method(shadowed) = &resolved.program.types[0].members[1] else {
            panic!()
        };
        assert_eq!(specialized_method(shadowed, &substitutions), *shadowed);
    }
}
