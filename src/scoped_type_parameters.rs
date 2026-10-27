//! Give colliding source binders distinct identities before resolution.
//!
//! JavaType still spells both nominal types and variables as names. A scoped,
//! capture-avoiding alpha rename is sufficient to keep nominal storage facts
//! from being applied to an unrelated binder. One immutable catalog is shared
//! by the full symbol program and its separately resolved primary/support units.
use crate::java_ir::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct BinderCatalog {
    nominal: BTreeSet<String>,
    binders: BTreeMap<String, BTreeSet<String>>,
}

impl BinderCatalog {
    /// Borrows a mutable AST solely to reuse the declaration walker. Collection
    /// does not mutate it or clone another whole source program.
    pub(crate) fn collect(program: &mut Program) -> Self {
        let mut catalog = Self::default();
        walk_program(program, &mut Collector(&mut catalog));
        catalog
    }

    pub(crate) fn collect_many(programs: &mut [&mut Program]) -> Self {
        let mut catalog = Self::default();
        for program in programs {
            walk_program(program, &mut Collector(&mut catalog));
        }
        catalog
    }

    pub(crate) fn normalize(&self, program: &mut Program) {
        if !self
            .binders
            .keys()
            .any(|name| self.nominal.contains(name) || name.starts_with("__JavaTypeParam_"))
        {
            return;
        }
        walk_program(program, &mut Normalizer(self, BTreeSet::new()));
    }

    fn fresh(&self, scope: &str, index: usize, old: &str) -> String {
        let identity = format!("{scope}/parameter:{index}");
        // An injective spelling avoids depending on hash collision luck for
        // semantic identity. This only affects binders that actually collide.
        use std::fmt::Write;
        let mut encoded = String::new();
        for byte in identity.bytes().chain([0]).chain(old.bytes()) {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        for attempt in 0_u64.. {
            let name = format!("__JavaTypeParam_{encoded}_{attempt}");
            // A second catalog can see an already-normalized copy of this
            // very binder. Reusing its identity is safe; another lexical
            // binder or any nominal declaration must never be captured.
            let captured = self
                .binders
                .get(&name)
                .is_some_and(|owners| owners.iter().any(|owner| owner != &identity));
            if !self.nominal.contains(&name) && !captured {
                return name;
            }
        }
        unreachable!()
    }
}

trait Declarations {
    fn declaration(&mut self, ty: &mut TypeDecl, scope: &str, values: &crate::types::TypeEnv);
    fn method(&mut self, method: &mut MethodDecl, scope: &str, values: &crate::types::TypeEnv);
    fn constructor(
        &mut self,
        constructor: &mut ConstructorDecl,
        scope: &str,
        values: &crate::types::TypeEnv,
    );
    fn expression(&mut self, _expr: &mut Expr) {}
}

struct Collector<'a>(&'a mut BinderCatalog);
impl Collector<'_> {
    fn parameters(&mut self, parameters: &[String], scope: &str) {
        for (index, name) in parameters.iter().enumerate() {
            self.0
                .binders
                .entry(name.clone())
                .or_default()
                .insert(format!("{scope}/parameter:{index}"));
        }
    }
}
impl Declarations for Collector<'_> {
    fn declaration(&mut self, ty: &mut TypeDecl, scope: &str, _values: &crate::types::TypeEnv) {
        self.0.nominal.insert(ty.name.clone());
        self.0
            .nominal
            .insert(ty.name.rsplit('.').next().unwrap_or(&ty.name).into());
        if let Some(names) = &ty.source_names {
            self.0.nominal.insert(names.canonical.clone());
            self.0.nominal.insert(
                names
                    .canonical
                    .rsplit('.')
                    .next()
                    .unwrap_or(&names.canonical)
                    .into(),
            );
        }
        self.parameters(&ty.type_params, scope);
    }
    fn method(&mut self, method: &mut MethodDecl, scope: &str, _values: &crate::types::TypeEnv) {
        self.parameters(&method.type_params, scope);
    }
    fn constructor(
        &mut self,
        constructor: &mut ConstructorDecl,
        scope: &str,
        _values: &crate::types::TypeEnv,
    ) {
        self.parameters(&constructor.type_params, scope);
    }
}

struct Normalizer<'a>(&'a BinderCatalog, BTreeSet<String>);
impl Declarations for Normalizer<'_> {
    fn declaration(&mut self, ty: &mut TypeDecl, scope: &str, values: &crate::types::TypeEnv) {
        for index in 0..ty.type_params.len() {
            let old = ty.type_params[index].clone();
            if self.0.nominal.contains(&old) {
                let fresh = self.0.fresh(scope, index, &old);
                self.1.insert(fresh.clone());
                crate::java_type_substitution::rename_class_parameter_in_value_scope(
                    ty, index, fresh, values,
                );
            }
        }
    }
    fn method(&mut self, method: &mut MethodDecl, scope: &str, values: &crate::types::TypeEnv) {
        for index in 0..method.type_params.len() {
            let old = method.type_params[index].clone();
            if self.0.nominal.contains(&old) {
                let fresh = self.0.fresh(scope, index, &old);
                self.1.insert(fresh.clone());
                crate::java_type_substitution::rename_method_parameter_in_value_scope(
                    method, index, fresh, values,
                );
            }
        }
    }
    fn constructor(
        &mut self,
        constructor: &mut ConstructorDecl,
        scope: &str,
        values: &crate::types::TypeEnv,
    ) {
        for index in 0..constructor.type_params.len() {
            let old = constructor.type_params[index].clone();
            if self.0.nominal.contains(&old) {
                let fresh = self.0.fresh(scope, index, &old);
                self.1.insert(fresh.clone());
                crate::java_type_substitution::rename_constructor_parameter_in_value_scope(
                    constructor,
                    index,
                    fresh,
                    values,
                );
            }
        }
    }
    fn expression(&mut self, expr: &mut Expr) {
        if matches!(expr, Expr::MethodReference { target, .. } if self.1.contains(target)) {
            // The parser's bare method-reference target can denote either a
            // type variable or a same-spelled value. Alpha-renaming without
            // that lexical namespace proof would silently select another
            // callable. Keep this boundary explicit until resolver-owned
            // method-reference identity can be carried through normalization.
            *expr = Expr::Unsupported {
                reason: "method reference through a renamed type parameter requires a lexical type/value target proof".into(),
                span: crate::diagnostics::Span::default(),
            };
        }
    }
}

fn walk_program(program: &mut Program, visitor: &mut impl Declarations) {
    let values = &crate::types::TypeEnv::new();
    let package = program.package.as_deref().unwrap_or("");
    for ty in &mut program.types {
        walk_type(ty, package, visitor, values);
    }
}
fn walk_type(
    ty: &mut TypeDecl,
    parent: &str,
    visitor: &mut impl Declarations,
    values: &crate::types::TypeEnv,
) {
    let scope = ty
        .source_names
        .as_ref()
        .map(|names| names.canonical.clone())
        .unwrap_or_else(|| {
            if parent.is_empty() || ty.name.contains('.') {
                ty.name.clone()
            } else {
                format!("{parent}.{}", ty.name)
            }
        });
    let declaration_values =
        crate::java_type_substitution::value_scope_for_members(&ty.members, values);
    let values = &declaration_values;
    visitor.declaration(ty, &scope, values);
    walk_members(&mut ty.members, &scope, visitor, values);
    for (index, init) in ty.static_init_blocks.iter_mut().enumerate() {
        walk_block(
            &mut init.block,
            &format!("{scope}/initializer:{index}"),
            visitor,
            values,
        );
    }
}
fn walk_members(
    members: &mut [Member],
    parent: &str,
    visitor: &mut impl Declarations,
    values: &crate::types::TypeEnv,
) {
    for member in members {
        match member {
            Member::Type(ty) => walk_type(ty, parent, visitor, values),
            Member::Method(method) => {
                let scope = format!("{parent}/method:{}", method.name);
                visitor.method(method, &scope, values);
                let mut callable_values = values.clone();
                for param in &method.params {
                    crate::java_type_substitution::insert_value_binding(
                        &mut callable_values,
                        &param.name,
                        &param.source_name,
                        &param.ty,
                    );
                }
                if let Some(body) = &mut method.body {
                    walk_block(body, &scope, visitor, &callable_values);
                }
            }
            Member::Constructor(constructor) => {
                let scope = format!("{parent}/constructor:{}", constructor.params.len());
                visitor.constructor(constructor, &scope, values);
                let mut callable_values = values.clone();
                for param in &constructor.params {
                    crate::java_type_substitution::insert_value_binding(
                        &mut callable_values,
                        &param.name,
                        &param.source_name,
                        &param.ty,
                    );
                }
                if let Some(body) = &mut constructor.body {
                    walk_block(body, &scope, visitor, &callable_values);
                }
            }
            Member::Field(field) => {
                if let Some(value) = &mut field.initializer {
                    walk_expr(
                        value,
                        &format!("{parent}/field:{}", field.name),
                        visitor,
                        values,
                    );
                }
            }
            Member::Unsupported { .. } => {}
        }
    }
}
fn walk_block(
    block: &mut Block,
    scope: &str,
    visitor: &mut impl Declarations,
    values: &crate::types::TypeEnv,
) {
    let mut block_values = values.clone();
    for (index, stmt) in block.statements.iter_mut().enumerate() {
        if let Stmt::VarDecl {
            name,
            source_name,
            ty,
            ..
        } = stmt
        {
            crate::java_type_substitution::insert_value_binding(
                &mut block_values,
                name,
                source_name,
                ty,
            );
        }
        walk_stmt(
            stmt,
            &format!("{scope}/statement:{index}"),
            visitor,
            &block_values,
        );
    }
}
fn walk_stmt(
    stmt: &mut Stmt,
    scope: &str,
    visitor: &mut impl Declarations,
    values: &crate::types::TypeEnv,
) {
    match stmt {
        Stmt::LocalType(ty) => walk_type(ty, scope, visitor, values),
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                walk_expr(expr, scope, visitor, values);
            }
        }
        Stmt::Assign { target, value } => {
            walk_expr(target, scope, visitor, values);
            walk_expr(value, scope, visitor, values);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            walk_expr(expr, scope, visitor, values)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            walk_expr(condition, scope, visitor, values);
            walk_block(then_block, &format!("{scope}/then"), visitor, values);
            if let Some(body) = else_block {
                walk_block(body, &format!("{scope}/else"), visitor, values);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            walk_expr(condition, scope, visitor, values);
            walk_block(body, scope, visitor, values);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            source_var_name,
            iterable,
            body,
            ..
        } => {
            walk_expr(iterable, scope, visitor, values);
            let mut loop_values = values.clone();
            crate::java_type_substitution::insert_value_binding(
                &mut loop_values,
                var_name,
                source_var_name,
                var_type,
            );
            walk_block(body, scope, visitor, &loop_values);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_values = values.clone();
            for stmt in init {
                if let Stmt::VarDecl {
                    name,
                    source_name,
                    ty,
                    ..
                } = stmt
                {
                    crate::java_type_substitution::insert_value_binding(
                        &mut loop_values,
                        name,
                        source_name,
                        ty,
                    );
                }
                walk_stmt(stmt, scope, visitor, &loop_values);
            }
            if let Some(expr) = condition {
                walk_expr(expr, scope, visitor, &loop_values);
            }
            for expr in update {
                walk_expr(expr, scope, visitor, &loop_values);
            }
            walk_block(body, scope, visitor, &loop_values);
        }
        Stmt::Switch { selector, cases } => {
            walk_expr(selector, scope, visitor, values);
            let mut case_values = values.clone();
            for (index, case) in cases.iter_mut().enumerate() {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        walk_expr(expr, scope, visitor, values);
                    }
                }
                for stmt in &mut case.statements {
                    if let Stmt::VarDecl {
                        name,
                        source_name,
                        ty,
                        ..
                    } = stmt
                    {
                        crate::java_type_substitution::insert_value_binding(
                            &mut case_values,
                            name,
                            source_name,
                            ty,
                        );
                    }
                    walk_stmt(
                        stmt,
                        &format!("{scope}/case:{index}"),
                        visitor,
                        &case_values,
                    );
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
            for stmt in resources {
                if let Stmt::VarDecl {
                    name,
                    source_name,
                    ty,
                    ..
                } = stmt
                {
                    crate::java_type_substitution::insert_value_binding(
                        &mut resource_values,
                        name,
                        source_name,
                        ty,
                    );
                }
                walk_stmt(stmt, scope, visitor, &resource_values);
            }
            walk_block(body, scope, visitor, &resource_values);
            for (index, catch) in catches.iter_mut().enumerate() {
                let mut catch_values = values.clone();
                let ty = catch
                    .types
                    .first()
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown(catch.name.clone()));
                crate::java_type_substitution::insert_value_binding(
                    &mut catch_values,
                    &catch.name,
                    &catch.source_name,
                    &ty,
                );
                walk_block(
                    &mut catch.body,
                    &format!("{scope}/catch:{index}"),
                    visitor,
                    &catch_values,
                );
            }
            if let Some(body) = finally_block {
                walk_block(body, &format!("{scope}/finally"), visitor, values);
            }
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            walk_block(body, scope, visitor, values)
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for expr in args {
                walk_expr(expr, scope, visitor, values);
            }
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}
fn walk_expr(
    expr: &mut Expr,
    scope: &str,
    visitor: &mut impl Declarations,
    values: &crate::types::TypeEnv,
) {
    visitor.expression(expr);
    match expr {
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            if let Some(expr) = qualifier {
                walk_expr(expr, scope, visitor, values);
            }
            for (index, expr) in args.iter_mut().enumerate() {
                walk_expr(expr, &format!("{scope}/argument:{index}"), visitor, values);
            }
            if let Some(members) = body {
                let anonymous_values =
                    crate::java_type_substitution::value_scope_for_members(members, values);
                walk_members(
                    members,
                    &format!("{scope}/anonymous"),
                    visitor,
                    &anonymous_values,
                );
            }
        }
        Expr::FieldAccess(expr, _)
        | Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. } => walk_expr(expr, scope, visitor, values),
        Expr::MethodReference { receiver, .. } => {
            if let Some(expr) = receiver {
                walk_expr(expr, scope, visitor, values);
            }
        }
        Expr::ArrayAccess(left, right) | Expr::Binary { left, right, .. } => {
            walk_expr(left, &format!("{scope}/left"), visitor, values);
            walk_expr(right, &format!("{scope}/right"), visitor, values);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                walk_expr(expr, scope, visitor, values);
            }
            if let Some(elements) = initializer {
                for expr in elements {
                    walk_expr(expr, scope, visitor, values);
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for expr in elements {
                walk_expr(expr, scope, visitor, values);
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            walk_expr(condition, scope, visitor, values);
            walk_expr(then_expr, &format!("{scope}/then"), visitor, values);
            walk_expr(else_expr, &format!("{scope}/else"), visitor, values);
        }
        Expr::Switch { selector, cases } => {
            walk_expr(selector, scope, visitor, values);
            for (index, case) in cases.iter_mut().enumerate() {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        walk_expr(expr, scope, visitor, values);
                    }
                }
                walk_expr(
                    &mut case.value,
                    &format!("{scope}/case:{index}"),
                    visitor,
                    values,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(expr) = receiver {
                walk_expr(expr, scope, visitor, values);
            }
            for (index, expr) in args.iter_mut().enumerate() {
                walk_expr(expr, &format!("{scope}/argument:{index}"), visitor, values);
            }
        }
        Expr::Lambda { params, body } => {
            let mut lambda_values = values.clone();
            for param in params {
                let ty = param
                    .ty
                    .clone()
                    .unwrap_or_else(|| JavaType::Unknown(param.name.clone()));
                crate::java_type_substitution::insert_value_binding(
                    &mut lambda_values,
                    &param.name,
                    &param.source_name,
                    &ty,
                );
            }
            match body {
                LambdaBody::Expr(expr) => walk_expr(expr, scope, visitor, &lambda_values),
                LambdaBody::Block(body) => walk_block(body, scope, visitor, &lambda_values),
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn named(ty: &JavaType) -> &str {
        match ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
            _ => panic!("expected named type: {ty:?}"),
        }
    }
    #[test]
    fn class_callable_constructor_and_nested_binders_keep_distinct_scopes() {
        let mut program = crate::parser::parse_java_source("class Cell {} class Box<Cell> { Cell value; <Cell> Cell own(Cell input) { return input; } <Cell> Box(Cell input) {} static class Nested<Cell> { Cell nested; } }").program;
        let catalog = BinderCatalog::collect(&mut program);
        catalog.normalize(&mut program);
        let owner = &program.types[1];
        let class = &owner.type_params[0];
        assert_ne!(class, "Cell");
        let Member::Field(field) = &owner.members[0] else {
            panic!()
        };
        assert_eq!(named(&field.ty), class);
        let Member::Method(method) = &owner.members[1] else {
            panic!()
        };
        assert_ne!(&method.type_params[0], class);
        assert_eq!(named(&method.params[0].ty), method.type_params[0]);
        assert_eq!(named(&method.return_type), method.type_params[0]);
        let Member::Constructor(ctor) = &owner.members[2] else {
            panic!()
        };
        assert_ne!(&ctor.type_params[0], class);
        assert_ne!(ctor.type_params[0], method.type_params[0]);
        assert_eq!(named(&ctor.params[0].ty), ctor.type_params[0]);
        let Member::Type(nested) = &owner.members[3] else {
            panic!()
        };
        assert_ne!(&nested.type_params[0], class);
        let once = program.clone();
        catalog.normalize(&mut program);
        assert_eq!(program, once);
    }
    #[test]
    fn separate_source_units_and_explicit_method_witnesses_use_the_same_identity() {
        let source = "class Box<Cell> { <X> X identity(X value) { return value; } Cell relay(Cell value) { return this.<Cell>identity(value); } }";
        let mut unit = crate::parser::parse_java_source(source).program;
        let mut full =
            crate::parser::parse_java_source(&format!("class Cell {{}} {source}")).program;
        let symbols = crate::resolve::source_symbols_from_program(&full);
        symbols.normalize_type_parameters(&mut unit);
        symbols.normalize_type_parameters(&mut full);
        let mut expected = full.types[1].clone();
        expected.span = unit.types[0].span;
        for (left, right) in unit.types[0].members.iter().zip(&mut expected.members) {
            match (left, right) {
                (Member::Method(a), Member::Method(b)) => b.span = a.span,
                (Member::Field(a), Member::Field(b)) => b.span = a.span,
                (Member::Constructor(a), Member::Constructor(b)) => b.span = a.span,
                _ => {}
            }
        }
        assert_eq!(unit.types[0], expected);
        let owner = &unit.types[0];
        let Member::Method(method) = &owner.members[1] else {
            panic!()
        };
        let Stmt::Return(Some(Expr::Call { type_args, .. })) =
            &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("explicit generic call")
        };
        assert_eq!(named(&type_args[0]), owner.type_params[0]);
        // Rebuilding a catalog on normalized support must not rename its copy
        // differently from an unresolved primary unit of the same declaration.
        let mut raw = crate::parser::parse_java_source(source).program;
        let again = crate::resolve::source_symbols_from_program(&full);
        again.normalize_type_parameters(&mut raw);
        assert_eq!(unit, raw);
    }
    #[test]
    fn ambiguous_method_reference_targets_are_loud_until_namespace_identity_is_available() {
        let mut program = crate::parser::parse_java_source("class Cell {} class Owner<Cell> { Object typeTarget() { return Cell::toString; } Object valueTarget(Cell Cell) { return Cell::toString; } }").program;
        let catalog = BinderCatalog::collect(&mut program);
        catalog.normalize(&mut program);
        let Member::Method(by_type) = &program.types[1].members[0] else {
            panic!()
        };
        assert!(
            matches!(&by_type.body.as_ref().unwrap().statements[0], Stmt::Return(Some(Expr::Unsupported { reason, .. })) if reason.contains("lexical type/value target proof"))
        );
        let Member::Method(by_value) = &program.types[1].members[1] else {
            panic!()
        };
        assert!(
            matches!(&by_value.body.as_ref().unwrap().statements[0], Stmt::Return(Some(Expr::MethodReference { target, .. })) if target == "Cell")
        );
    }

    #[test]
    fn value_target_names_follow_block_lambda_loop_catch_and_nested_scopes() {
        let mut program = crate::parser::parse_java_source(r#"
class Cell {}
class FieldOwner<Cell> {
    java.util.function.IntSupplier Cell;
    java.util.function.IntSupplier byField() { return Cell::getAsInt; }
}
class Owner<Cell> {
    Object scopes(java.util.function.IntSupplier input, java.util.function.IntSupplier[] all) {
        { java.util.function.IntSupplier Cell = input; java.util.function.IntSupplier a = Cell::getAsInt; }
        java.util.function.Function<java.util.function.IntSupplier, java.util.function.IntSupplier> f = (Cell) -> Cell::getAsInt;
        for (java.util.function.IntSupplier Cell : all) { java.util.function.IntSupplier a = Cell::getAsInt; }
        for (java.util.function.IntSupplier Cell = input; false;) { java.util.function.IntSupplier a = Cell::getAsInt; }
        try {} catch (RuntimeException Cell) { java.util.function.Supplier<String> a = Cell::getMessage; }
        class Local { <Cell> java.util.function.IntSupplier fromCapture() { return input::getAsInt; } }
        return Cell::toString;
    }
}
"#).program;
        let catalog = BinderCatalog::collect(&mut program);
        catalog.normalize(&mut program);
        let mut targets = Vec::new();
        let mut unsupported = 0;
        crate::semantic_analysis::walk_program_exprs(&program, &mut |expr| match expr {
            Expr::MethodReference { target, .. } => targets.push(target.clone()),
            Expr::Unsupported { reason, .. }
                if reason.contains("lexical type/value target proof") =>
            {
                unsupported += 1
            }
            _ => {}
        });
        assert_eq!(
            targets
                .iter()
                .filter(|target| target.as_str() == "Cell")
                .count(),
            6,
            "{targets:?}"
        );
        assert!(targets.iter().any(|target| target == "input"));
        // No value named Cell escapes its lexical block/loop/lambda/catch.
        assert_eq!(unsupported, 1);
    }

    #[test]
    fn local_and_anonymous_callable_binders_are_normalized_before_lifting() {
        let mut program = crate::parser::parse_java_source("class Cell {} class Use { Object build() { class Local<Cell> { Cell item; } return new Object() { <Cell> Cell id(Cell input) { return input; } }; } }").program;
        let catalog = BinderCatalog::collect(&mut program);
        catalog.normalize(&mut program);
        let Member::Method(method) = &program.types[1].members[0] else {
            panic!()
        };
        let body = method.body.as_ref().unwrap();
        let Stmt::LocalType(local) = &body.statements[0] else {
            panic!("local class")
        };
        assert_ne!(local.type_params[0], "Cell");
        let Stmt::Return(Some(Expr::New {
            body: Some(members),
            ..
        })) = &body.statements[1]
        else {
            panic!("anonymous object")
        };
        let Member::Method(method) = &members[0] else {
            panic!()
        };
        assert_ne!(method.type_params[0], "Cell");
        assert_eq!(named(&method.params[0].ty), method.type_params[0]);
    }
}
