//! Binder-aware specialization of source members, including their bodies.
//! Binding IDs and declaration locations are retained; only type occurrences
//! change. A member/local class's own type parameters shadow enclosing ones.
use crate::java_ir::*;
use std::{borrow::Cow, collections::HashMap};

type Substitutions = HashMap<String, JavaType>;

pub(crate) fn rename_class_parameter(declaration: &mut TypeDecl, index: usize, new_name: String) {
    rename_class_parameter_in_value_scope(
        declaration,
        index,
        new_name,
        &crate::types::TypeEnv::new(),
    );
}

pub(crate) fn rename_class_parameter_in_value_scope(
    declaration: &mut TypeDecl,
    index: usize,
    new_name: String,
    values: &crate::types::TypeEnv,
) {
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
        values,
    );
}

pub(crate) fn rename_method_parameter(method: &mut MethodDecl, index: usize, new_name: String) {
    rename_method_parameter_in_value_scope(method, index, new_name, &crate::types::TypeEnv::new());
}

pub(crate) fn rename_method_parameter_in_value_scope(
    method: &mut MethodDecl,
    index: usize,
    new_name: String,
    values: &crate::types::TypeEnv,
) {
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
        values,
    );
}

pub(crate) fn rename_constructor_parameter(
    constructor: &mut ConstructorDecl,
    index: usize,
    new_name: String,
) {
    rename_constructor_parameter_in_value_scope(
        constructor,
        index,
        new_name,
        &crate::types::TypeEnv::new(),
    );
}

pub(crate) fn rename_constructor_parameter_in_value_scope(
    constructor: &mut ConstructorDecl,
    index: usize,
    new_name: String,
    values: &crate::types::TypeEnv,
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
        values,
    );
}

pub(crate) fn specialized_method(method: &MethodDecl, substitutions: &Substitutions) -> MethodDecl {
    let mut result = method.clone();
    let values = &crate::types::TypeEnv::new();
    method_types(&mut result, substitutions, values);
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

pub(crate) fn insert_value_binding(
    values: &mut crate::types::TypeEnv,
    name: &str,
    source_name: &str,
    ty: &JavaType,
) {
    values.insert_local(name, ty.clone());
    values.insert_local(source_name, ty.clone());
}

pub(crate) fn value_scope_for_members(
    members: &[Member],
    outer: &crate::types::TypeEnv,
) -> crate::types::TypeEnv {
    let mut values = outer.clone();
    for item in members {
        if let Member::Field(field) = item {
            values.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    values
}

fn method_types(
    method: &mut MethodDecl,
    substitutions: &Substitutions,
    values: &crate::types::TypeEnv,
) {
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
    let mut callable_values = values.clone();
    for param in &method.params {
        insert_value_binding(
            &mut callable_values,
            &param.name,
            &param.source_name,
            &param.ty,
        );
    }
    let values = &callable_values;
    if let Some(body) = &mut method.body {
        block(body, &substitutions, values);
    }
}

fn member(member: &mut Member, substitutions: &Substitutions, values: &crate::types::TypeEnv) {
    match member {
        Member::Method(method) => method_types(method, substitutions, values),
        Member::Field(field) => {
            ty(&mut field.ty, substitutions);
            if let Some(value) = &mut field.initializer {
                expr(value, substitutions, values);
            }
        }
        Member::Constructor(constructor) => constructor_types(constructor, substitutions, values),
        Member::Type(declaration) => declaration_types(declaration, substitutions, values),
        Member::Unsupported { .. } => {}
    }
}

fn constructor_types(
    constructor: &mut ConstructorDecl,
    substitutions: &Substitutions,
    values: &crate::types::TypeEnv,
) {
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
    let mut callable_values = values.clone();
    for param in &constructor.params {
        insert_value_binding(
            &mut callable_values,
            &param.name,
            &param.source_name,
            &param.ty,
        );
    }
    let values = &callable_values;
    if let Some(body) = &mut constructor.body {
        block(body, &substitutions, values);
    }
}

fn declaration_types(
    declaration: &mut TypeDecl,
    substitutions: &Substitutions,
    values: &crate::types::TypeEnv,
) {
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
    declaration_contents(declaration, &substitutions, values);
}

fn declaration_contents(
    declaration: &mut TypeDecl,
    substitutions: &Substitutions,
    values: &crate::types::TypeEnv,
) {
    let declaration_values = value_scope_for_members(&declaration.members, values);
    let values = &declaration_values;
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
        member(item, &substitutions, values);
    }
    for init in &mut declaration.static_init_blocks {
        block(&mut init.block, &substitutions, values);
    }
}

fn block(body: &mut Block, substitutions: &Substitutions, values: &crate::types::TypeEnv) {
    let mut scope = Cow::Borrowed(substitutions);
    let mut block_values = values.clone();
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
        if let Stmt::VarDecl {
            name,
            source_name,
            ty,
            ..
        } = statement
        {
            insert_value_binding(&mut block_values, name, source_name, ty);
        }
        stmt(statement, &scope, &block_values);
    }
}

fn stmt(statement: &mut Stmt, substitutions: &Substitutions, values: &crate::types::TypeEnv) {
    match statement {
        Stmt::VarDecl {
            ty: declared,
            initializer,
            ..
        } => {
            ty(declared, substitutions);
            if let Some(value) = initializer {
                expr(value, substitutions, values);
            }
        }
        Stmt::Assign { target, value } => {
            expr(target, substitutions, values);
            expr(value, substitutions, values);
        }
        Stmt::Expr(value) | Stmt::Return(Some(value)) | Stmt::Throw(value) => {
            expr(value, substitutions, values)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr(condition, substitutions, values);
            block(then_block, substitutions, values);
            if let Some(body) = else_block {
                block(body, substitutions, values);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr(condition, substitutions, values);
            block(body, substitutions, values);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_values = values.clone();
            for statement in init {
                if let Stmt::VarDecl {
                    name,
                    source_name,
                    ty,
                    ..
                } = statement
                {
                    insert_value_binding(&mut loop_values, name, source_name, ty);
                }
                stmt(statement, substitutions, &loop_values);
            }
            if let Some(value) = condition {
                expr(value, substitutions, &loop_values);
            }
            for value in update {
                expr(value, substitutions, &loop_values);
            }
            block(body, substitutions, &loop_values);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            source_var_name,
            iterable,
            body,
            ..
        } => {
            ty(var_type, substitutions);
            expr(iterable, substitutions, values);
            let mut loop_values = values.clone();
            insert_value_binding(&mut loop_values, var_name, source_var_name, var_type);
            block(body, substitutions, &loop_values);
        }
        Stmt::Switch { selector, cases } => {
            expr(selector, substitutions, values);
            let mut case_values = values.clone();
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(value) = label {
                        expr(value, substitutions, values);
                    }
                }
                for statement in &mut case.statements {
                    if let Stmt::VarDecl {
                        name,
                        source_name,
                        ty,
                        ..
                    } = statement
                    {
                        insert_value_binding(&mut case_values, name, source_name, ty);
                    }
                    stmt(statement, substitutions, &case_values);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut resource_values = values.clone();
            for statement in resources {
                if let Stmt::VarDecl {
                    name,
                    source_name,
                    ty,
                    ..
                } = statement
                {
                    insert_value_binding(&mut resource_values, name, source_name, ty);
                }
                stmt(statement, substitutions, &resource_values);
            }
            block(body, substitutions, &resource_values);
            for catch in catches {
                for caught in &mut catch.types {
                    ty(caught, substitutions);
                }
                let mut catch_values = values.clone();
                let caught = catch
                    .types
                    .first()
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown(catch.name.clone()));
                insert_value_binding(&mut catch_values, &catch.name, &catch.source_name, &caught);
                block(&mut catch.body, substitutions, &catch_values);
            }
            if let Some(body) = finally_block {
                block(body, substitutions, values);
            }
        }
        Stmt::LocalType(declaration) => declaration_types(declaration, substitutions, values),
        Stmt::ConstructorInvocation { args, resolved, .. } => {
            for value in args {
                expr(value, substitutions, values);
            }
            if let Some(call) = resolved {
                constructor(call, substitutions);
            }
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => block(body, substitutions, values),
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

fn expr(value: &mut Expr, substitutions: &Substitutions, values: &crate::types::TypeEnv) {
    match value {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr(receiver, substitutions, values)
        }
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            expr(left, substitutions, values);
            expr(right, substitutions, values);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            ty(element_type, substitutions);
            for value in dimensions {
                expr(value, substitutions, values);
            }
            if let Some(elements) = initializer {
                for value in elements {
                    expr(value, substitutions, values);
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for value in elements {
                expr(value, substitutions, values);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr(condition, substitutions, values);
            expr(then_expr, substitutions, values);
            expr(else_expr, substitutions, values);
        }
        Expr::Switch { selector, cases } => {
            expr(selector, substitutions, values);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(value) = label {
                        expr(value, substitutions, values);
                    }
                }
                expr(&mut case.value, substitutions, values);
            }
        }
        Expr::Call {
            receiver,
            type_args,
            args,
            resolved,
            ..
        } => {
            for argument in type_args {
                ty(argument, substitutions);
            }
            if let Some(receiver) = receiver {
                expr(receiver, substitutions, values);
            }
            for value in args {
                expr(value, substitutions, values);
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
                expr(value, substitutions, values);
            }
            for value in args {
                expr(value, substitutions, values);
            }
            if let Some(body) = body {
                let anonymous_values = value_scope_for_members(body, values);
                for item in body {
                    member(item, substitutions, &anonymous_values);
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
            expr(value, substitutions, values);
        }
        Expr::Lambda { params, body } => {
            for param in params.iter_mut() {
                if let Some(declared) = &mut param.ty {
                    ty(declared, substitutions);
                }
            }
            let mut lambda_values = values.clone();
            for param in params.iter() {
                let ty = param
                    .ty
                    .clone()
                    .unwrap_or_else(|| JavaType::Unknown(param.name.clone()));
                insert_value_binding(&mut lambda_values, &param.name, &param.source_name, &ty);
            }
            let values = &lambda_values;
            match body {
                LambdaBody::Expr(value) => expr(value, substitutions, values),
                LambdaBody::Block(body) => block(body, substitutions, values),
            }
        }
        Expr::MethodReference {
            target, receiver, ..
        } => {
            if let Some(receiver) = receiver {
                expr(receiver, substitutions, values);
            }
            if values
                .method_reference_value_binding(target, true)
                .is_none()
            {
                if let Some(declared) = substitutions.get(target) {
                    *target = java_type_text(declared);
                }
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
