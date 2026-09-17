use crate::diagnostics::{DiagnosticBag, Span};
use crate::java_ir::*;

#[derive(Debug, Clone)]
pub struct ParsedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
}

pub fn parse_java_source(source: &str) -> ParsedProgram {
    let mut diagnostics = DiagnosticBag::new();
    let Some(tree) = parse_tree(source, &mut diagnostics) else {
        return ParsedProgram {
            program: Program::default(),
            diagnostics,
        };
    };
    let root = tree.root_node();
    collect_syntax_diagnostics(root, &mut diagnostics);

    let imports = parse_imports(root, source);
    let mut types = parse_top_level_types(root, source, &mut diagnostics);
    qualify_java_sql_imported_types(&mut types, &imports);

    ParsedProgram {
        program: Program {
            package: parse_package(root, source),
            imports,
            types,
        },
        diagnostics,
    }
}

fn parse_tree(source: &str, diagnostics: &mut DiagnosticBag) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("load tree-sitter-java");
    let tree = parser.parse(source, None);
    if tree.is_none() {
        diagnostics.error("parse_tree_sitter", "tree-sitter failed to parse source");
    }
    tree
}

fn parse_package(root: tree_sitter::Node, source: &str) -> Option<String> {
    named_children(root)
        .into_iter()
        .find(|node| node.kind() == "package_declaration")
        .map(|node| {
            let raw = text(node, source).trim();
            raw.rsplit_once("package")
                .map(|(_, package)| package)
                .unwrap_or(raw)
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_string()
        })
}

fn parse_imports(root: tree_sitter::Node, source: &str) -> Vec<ImportDecl> {
    named_children(root)
        .into_iter()
        .filter(|node| node.kind() == "import_declaration")
        .map(|node| {
            let raw = text(node, source);
            let raw = raw
                .trim()
                .trim_start_matches("import")
                .trim()
                .trim_end_matches(';')
                .trim();
            let (is_static, path) = raw
                .strip_prefix("static ")
                .map(|path| (true, path.trim()))
                .unwrap_or((false, raw));
            ImportDecl {
                path: path.trim_end_matches(".*").to_string(),
                is_static,
                is_wildcard: path.ends_with(".*"),
            }
        })
        .collect()
}

fn qualify_java_sql_imported_types(types: &mut [TypeDecl], imports: &[ImportDecl]) {
    let imported = JavaSqlImports::from_imports(imports);
    if imported.is_empty() {
        return;
    }
    for ty in types {
        qualify_java_sql_type_decl(ty, &imported);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct JavaSqlImports {
    wildcard: bool,
    connection: bool,
    statement: bool,
    prepared_statement: bool,
    result_set: bool,
    result_set_metadata: bool,
    database_metadata: bool,
    sql_exception: bool,
    driver_manager: bool,
}

impl JavaSqlImports {
    fn from_imports(imports: &[ImportDecl]) -> Self {
        let mut imported = Self::default();
        for import in imports.iter().filter(|import| !import.is_static) {
            if import.is_wildcard && import.path == "java.sql" {
                imported.wildcard = true;
                continue;
            }
            match import.path.as_str() {
                "java.sql.Connection" => imported.connection = true,
                "java.sql.Statement" => imported.statement = true,
                "java.sql.PreparedStatement" => imported.prepared_statement = true,
                "java.sql.ResultSet" => imported.result_set = true,
                "java.sql.ResultSetMetaData" => imported.result_set_metadata = true,
                "java.sql.DatabaseMetaData" => imported.database_metadata = true,
                "java.sql.SQLException" => imported.sql_exception = true,
                "java.sql.DriverManager" => imported.driver_manager = true,
                _ => {}
            }
        }
        imported
    }

    fn is_empty(self) -> bool {
        !self.wildcard
            && !self.connection
            && !self.statement
            && !self.prepared_statement
            && !self.result_set
            && !self.result_set_metadata
            && !self.database_metadata
            && !self.sql_exception
            && !self.driver_manager
    }

    fn qualify(self, name: &str) -> Option<&'static str> {
        match name {
            "Connection" if self.wildcard || self.connection => Some("java.sql.Connection"),
            "Statement" if self.wildcard || self.statement => Some("java.sql.Statement"),
            "PreparedStatement" if self.wildcard || self.prepared_statement => {
                Some("java.sql.PreparedStatement")
            }
            "ResultSet" if self.wildcard || self.result_set => Some("java.sql.ResultSet"),
            "ResultSetMetaData" if self.wildcard || self.result_set_metadata => {
                Some("java.sql.ResultSetMetaData")
            }
            "DatabaseMetaData" if self.wildcard || self.database_metadata => {
                Some("java.sql.DatabaseMetaData")
            }
            "SQLException" if self.wildcard || self.sql_exception => Some("java.sql.SQLException"),
            "DriverManager" if self.wildcard || self.driver_manager => {
                Some("java.sql.DriverManager")
            }
            _ => None,
        }
    }
}

fn qualify_java_sql_type_decl(ty: &mut TypeDecl, imported: &JavaSqlImports) {
    for bound in &mut ty.type_param_bounds {
        if let Some(bound) = bound {
            qualify_java_sql_type(bound, imported);
        }
    }
    for member in &mut ty.members {
        qualify_java_sql_member(member, imported);
    }
}

fn qualify_java_sql_member(member: &mut Member, imported: &JavaSqlImports) {
    match member {
        Member::Field(field) => {
            qualify_java_sql_type(&mut field.ty, imported);
            if let Some(initializer) = &mut field.initializer {
                qualify_java_sql_expr(initializer, imported);
            }
        }
        Member::Method(method) => {
            for bound in &mut method.type_param_bounds {
                if let Some(bound) = bound {
                    qualify_java_sql_type(bound, imported);
                }
            }
            qualify_java_sql_type(&mut method.return_type, imported);
            for thrown in &mut method.throws {
                qualify_java_sql_type(thrown, imported);
            }
            for param in &mut method.params {
                qualify_java_sql_type(&mut param.ty, imported);
            }
            if let Some(body) = &mut method.body {
                qualify_java_sql_block(body, imported);
            }
        }
        Member::Constructor(ctor) => {
            for bound in &mut ctor.type_param_bounds {
                if let Some(bound) = bound {
                    qualify_java_sql_type(bound, imported);
                }
            }
            for thrown in &mut ctor.throws {
                qualify_java_sql_type(thrown, imported);
            }
            for param in &mut ctor.params {
                qualify_java_sql_type(&mut param.ty, imported);
            }
            if let Some(body) = &mut ctor.body {
                qualify_java_sql_block(body, imported);
            }
        }
        Member::Type(nested) => qualify_java_sql_type_decl(nested, imported),
        Member::Unsupported { .. } => {}
    }
}

fn qualify_java_sql_block(block: &mut Block, imported: &JavaSqlImports) {
    for stmt in &mut block.statements {
        qualify_java_sql_stmt(stmt, imported);
    }
}

fn qualify_java_sql_stmt(stmt: &mut Stmt, imported: &JavaSqlImports) {
    match stmt {
        Stmt::VarDecl {
            ty, initializer, ..
        } => {
            qualify_java_sql_type(ty, imported);
            if let Some(initializer) = initializer {
                qualify_java_sql_expr(initializer, imported);
            }
        }
        Stmt::Assign { target, value } => {
            qualify_java_sql_expr(target, imported);
            qualify_java_sql_expr(value, imported);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            qualify_java_sql_expr(expr, imported);
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            qualify_java_sql_expr(condition, imported);
            qualify_java_sql_block(then_block, imported);
            if let Some(else_block) = else_block {
                qualify_java_sql_block(else_block, imported);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            qualify_java_sql_expr(condition, imported);
            qualify_java_sql_block(body, imported);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for init in init {
                qualify_java_sql_stmt(init, imported);
            }
            if let Some(condition) = condition {
                qualify_java_sql_expr(condition, imported);
            }
            for update in update {
                qualify_java_sql_expr(update, imported);
            }
            qualify_java_sql_block(body, imported);
        }
        Stmt::EnhancedFor {
            var_type,
            iterable,
            body,
            ..
        } => {
            qualify_java_sql_type(var_type, imported);
            qualify_java_sql_expr(iterable, imported);
            qualify_java_sql_block(body, imported);
        }
        Stmt::Switch { selector, cases } => {
            qualify_java_sql_expr(selector, imported);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_java_sql_expr(expr, imported);
                    }
                }
                for stmt in &mut case.statements {
                    qualify_java_sql_stmt(stmt, imported);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for resource in resources {
                qualify_java_sql_stmt(resource, imported);
            }
            qualify_java_sql_block(body, imported);
            for catch in catches {
                for ty in &mut catch.types {
                    qualify_java_sql_type(ty, imported);
                }
                qualify_java_sql_block(&mut catch.body, imported);
            }
            if let Some(finally_block) = finally_block {
                qualify_java_sql_block(finally_block, imported);
            }
        }
        Stmt::LocalType(ty) => qualify_java_sql_type_decl(ty, imported),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                qualify_java_sql_expr(arg, imported);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            qualify_java_sql_block(block, imported)
        }
    }
}

fn qualify_java_sql_expr(expr: &mut Expr, imported: &JavaSqlImports) {
    match expr {
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
        Expr::FieldAccess(receiver, _) => qualify_java_sql_expr(receiver, imported),
        Expr::ArrayAccess(array, index) => {
            qualify_java_sql_expr(array, imported);
            qualify_java_sql_expr(index, imported);
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            qualify_java_sql_type(element_type, imported);
            for dimension in dimensions {
                qualify_java_sql_expr(dimension, imported);
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    qualify_java_sql_expr(element, imported);
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for element in elements {
                qualify_java_sql_expr(element, imported);
            }
        }
        Expr::Binary { left, right, .. } => {
            qualify_java_sql_expr(left, imported);
            qualify_java_sql_expr(right, imported);
        }
        Expr::Unary { expr, .. } => qualify_java_sql_expr(expr, imported),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            qualify_java_sql_expr(condition, imported);
            qualify_java_sql_expr(then_expr, imported);
            qualify_java_sql_expr(else_expr, imported);
        }
        Expr::Switch { selector, cases } => {
            qualify_java_sql_expr(selector, imported);
            for case in cases {
                for label in &mut case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        qualify_java_sql_expr(expr, imported);
                    }
                }
                qualify_java_sql_expr(&mut case.value, imported);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                qualify_java_sql_expr(receiver, imported);
            }
            for arg in args {
                qualify_java_sql_expr(arg, imported);
            }
        }
        Expr::New { ty, args, body, .. } => {
            qualify_java_sql_type(ty, imported);
            for arg in args {
                qualify_java_sql_expr(arg, imported);
            }
            if let Some(body) = body {
                for member in body {
                    qualify_java_sql_member(member, imported);
                }
            }
        }
        Expr::Cast { ty, expr } | Expr::InstanceOf { expr, ty, .. } => {
            qualify_java_sql_type(ty, imported);
            qualify_java_sql_expr(expr, imported);
        }
        Expr::Lambda { params, body } => {
            for param in params {
                if let Some(ty) = &mut param.ty {
                    qualify_java_sql_type(ty, imported);
                }
            }
            match body {
                LambdaBody::Expr(expr) => qualify_java_sql_expr(expr, imported),
                LambdaBody::Block(block) => qualify_java_sql_block(block, imported),
            }
        }
        Expr::MethodReference { .. } => {}
        Expr::ClassLiteral(ty) => qualify_java_sql_type(ty, imported),
    }
}

fn qualify_java_sql_type(ty: &mut JavaType, imported: &JavaSqlImports) {
    match ty {
        JavaType::Array(inner) => qualify_java_sql_type(inner, imported),
        JavaType::Named { name, args } => {
            if let Some(qualified) = imported.qualify(name) {
                *name = qualified.to_string();
            }
            for arg in args {
                qualify_java_sql_type(arg, imported);
            }
        }
        JavaType::Wildcard(Some(bound)) => match bound {
            WildcardBound::Extends(inner) | WildcardBound::Super(inner) => {
                qualify_java_sql_type(inner, imported);
            }
        },
        JavaType::Void
        | JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Wildcard(None)
        | JavaType::Unknown(_) => {}
    }
}

fn parse_top_level_types(
    root: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Vec<TypeDecl> {
    let mut types = Vec::new();
    for node in named_children(root) {
        match node.kind() {
            "class_declaration" => types.push(parse_class_decl(node, source, diagnostics)),
            "interface_declaration" => types.push(parse_interface_decl(node, source, diagnostics)),
            "enum_declaration" => types.push(parse_enum_decl(node, source, diagnostics)),
            "record_declaration" => types.push(parse_record_decl(node, source, diagnostics)),
            "annotation_type_declaration" => {
                types.push(parse_annotation_type_decl(node, source, diagnostics));
            }
            _ => {}
        }
    }
    if types.is_empty() && has_non_package_declaration(root) {
        diagnostics.error("parse_no_class", "no top-level class found");
    }
    types
}

fn has_non_package_declaration(root: tree_sitter::Node) -> bool {
    named_children(root).into_iter().any(|node| {
        !matches!(
            node.kind(),
            "package_declaration" | "import_declaration" | "line_comment" | "block_comment"
        )
    })
}

fn unsupported_member_for_node(node: tree_sitter::Node) -> Option<Member> {
    if matches!(node.kind(), "line_comment" | "block_comment") {
        return None;
    }
    Some(Member::Unsupported {
        reason: format!("unsupported member node: {}", node.kind()),
        span: span(node),
    })
}

fn parse_class_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let name = field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string());
    let extends = node
        .child_by_field_name("superclass")
        .map(|superclass| type_name_text(superclass, source));
    let implements = node
        .child_by_field_name("interfaces")
        .map(|interfaces| parse_type_name_list(interfaces, source))
        .unwrap_or_default();

    let mut members = Vec::new();
    let mut static_init_blocks = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "field_declaration" => members.extend(parse_field_decl(child, source)),
                "static_initializer" => {
                    // Record the position so class initialization can run field
                    // initializers and static blocks in textual order.
                    static_init_blocks.push(crate::java_ir::StaticInitBlock {
                        member_index: members.len(),
                        block: parse_static_initializer_block(child, source, diagnostics),
                    });
                }
                "constructor_declaration" => {
                    members.push(Member::Constructor(parse_constructor_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "method_declaration" => {
                    members.push(Member::Method(parse_method_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "class_declaration" => {
                    members.push(Member::Type(parse_class_decl(child, source, diagnostics)));
                }
                "interface_declaration" => {
                    members.push(Member::Type(parse_interface_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "enum_declaration" => {
                    members.push(Member::Type(parse_enum_decl(child, source, diagnostics)));
                }
                "record_declaration" => {
                    members.push(Member::Type(parse_record_decl(child, source, diagnostics)));
                }
                "annotation_type_declaration" => {
                    members.push(Member::Type(parse_annotation_type_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                _ => {
                    if let Some(member) = unsupported_member_for_node(child) {
                        members.push(member);
                    }
                }
            }
        }
    }

    TypeDecl {
        kind: TypeKind::Class,
        name,
        type_params: parse_type_parameter_names(node, source),
        type_param_bounds: parse_type_parameter_bounds(node, source),
        modifiers: parse_modifiers(node, source),
        extends,
        implements,
        permits: parse_permits(node, source),
        members,
        static_init_blocks,
        span: span(node),
    }
}

fn parse_interface_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let mut members = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "constant_declaration" | "field_declaration" => {
                    members.extend(parse_field_decl(child, source));
                }
                "method_declaration" => {
                    members.push(Member::Method(parse_method_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "class_declaration" => {
                    members.push(Member::Type(parse_class_decl(child, source, diagnostics)));
                }
                "interface_declaration" => {
                    members.push(Member::Type(parse_interface_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "enum_declaration" => {
                    members.push(Member::Type(parse_enum_decl(child, source, diagnostics)));
                }
                "record_declaration" => {
                    members.push(Member::Type(parse_record_decl(child, source, diagnostics)));
                }
                "annotation_type_declaration" => {
                    members.push(Member::Type(parse_annotation_type_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                _ => {
                    if let Some(member) = unsupported_member_for_node(child) {
                        members.push(member);
                    }
                }
            }
        }
    }

    let mut super_interfaces = node
        .child_by_field_name("interfaces")
        .map(|interfaces| parse_type_name_list(interfaces, source))
        .or_else(|| {
            named_children(node)
                .into_iter()
                .find(|child| child.kind() == "extends_interfaces")
                .map(|interfaces| parse_type_name_list(interfaces, source))
        })
        .unwrap_or_default();
    let extends = super_interfaces.first().cloned();
    if !super_interfaces.is_empty() {
        super_interfaces.remove(0);
    }

    TypeDecl {
        static_init_blocks: Vec::new(),
        kind: TypeKind::Interface,
        name: field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string()),
        type_params: parse_type_parameter_names(node, source),
        type_param_bounds: parse_type_parameter_bounds(node, source),
        modifiers: parse_modifiers(node, source),
        extends,
        implements: super_interfaces,
        permits: parse_permits(node, source),
        members,
        span: span(node),
    }
}

fn parse_enum_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let name = field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string());
    let mut members = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            parse_enum_member_node(child, &name, source, diagnostics, &mut members);
        }
    }

    TypeDecl {
        static_init_blocks: Vec::new(),
        kind: TypeKind::Enum,
        name,
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        modifiers: parse_modifiers(node, source),
        extends: None,
        implements: node
            .child_by_field_name("interfaces")
            .map(|interfaces| parse_type_name_list(interfaces, source))
            .unwrap_or_default(),
        permits: Vec::new(),
        members,
        span: span(node),
    }
}

fn parse_record_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let name = field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string());
    let components = node
        .child_by_field_name("parameters")
        .map(|params| parse_params_node(params, source))
        .unwrap_or_default();
    let mut members = components
        .iter()
        .flat_map(|component| record_component_members(component, node))
        .collect::<Vec<_>>();
    let mut has_canonical_constructor = false;

    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "field_declaration" => members.extend(parse_field_decl(child, source)),
                "constructor_declaration" => {
                    let constructor = parse_constructor_decl(child, source, diagnostics);
                    has_canonical_constructor |=
                        record_constructor_is_canonical(&constructor, &components);
                    members.push(Member::Constructor(constructor));
                }
                "compact_constructor_declaration" => {
                    has_canonical_constructor = true;
                    members.push(Member::Constructor(record_canonical_constructor_decl(
                        &name,
                        &components,
                        Some(child),
                        source,
                        diagnostics,
                    )));
                }
                "method_declaration" => {
                    members.push(Member::Method(parse_method_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "class_declaration" => {
                    members.push(Member::Type(parse_class_decl(child, source, diagnostics)));
                }
                "interface_declaration" => {
                    members.push(Member::Type(parse_interface_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "enum_declaration" => {
                    members.push(Member::Type(parse_enum_decl(child, source, diagnostics)));
                }
                "record_declaration" => {
                    members.push(Member::Type(parse_record_decl(child, source, diagnostics)));
                }
                "annotation_type_declaration" => {
                    members.push(Member::Type(parse_annotation_type_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                _ => {
                    if let Some(member) = unsupported_member_for_node(child) {
                        members.push(member);
                    }
                }
            }
        }
    }
    if !has_canonical_constructor {
        members.push(Member::Constructor(record_canonical_constructor_decl(
            &name,
            &components,
            None,
            source,
            diagnostics,
        )));
    }

    TypeDecl {
        static_init_blocks: Vec::new(),
        kind: TypeKind::Record,
        name,
        type_params: parse_type_parameter_names(node, source),
        type_param_bounds: parse_type_parameter_bounds(node, source),
        modifiers: parse_modifiers(node, source),
        extends: None,
        implements: node
            .child_by_field_name("interfaces")
            .map(|interfaces| parse_type_name_list(interfaces, source))
            .unwrap_or_default(),
        permits: Vec::new(),
        members,
        span: span(node),
    }
}

fn record_component_members(component: &Param, node: tree_sitter::Node) -> Vec<Member> {
    let field = Member::Field(FieldDecl {
        name: component.name.clone(),
        ty: component.ty.clone(),
        modifiers: vec![Modifier::Private, Modifier::Final],
        initializer: None,
        span: span(node),
    });
    let accessor = Member::Method(MethodDecl {
        name: component.name.clone(),
        rust_name: None,
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        return_type: component.ty.clone(),
        throws: Vec::new(),
        params: Vec::new(),
        modifiers: vec![Modifier::Public],
        body: Some(Block {
            statements: vec![Stmt::Return(Some(Expr::FieldAccess(
                Box::new(Expr::This),
                component.name.clone(),
            )))],
        }),
        span: span(node),
    });
    vec![field, accessor]
}

fn record_canonical_constructor_decl(
    record_name: &str,
    components: &[Param],
    node: Option<tree_sitter::Node>,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> ConstructorDecl {
    let mut body = node
        .and_then(|node| node.child_by_field_name("body"))
        .map(|body| parse_block_like(body, source, diagnostics))
        .unwrap_or_default();
    body.statements
        .extend(record_component_assignment_stmts(components));
    ConstructorDecl {
        name: record_name.to_string(),
        rust_name: None,
        type_params: node
            .map(|node| parse_type_parameter_names(node, source))
            .unwrap_or_default(),
        type_param_bounds: node
            .map(|node| parse_type_parameter_bounds(node, source))
            .unwrap_or_default(),
        params: components.to_vec(),
        throws: node
            .map(|node| parse_throws(node, source))
            .unwrap_or_default(),
        modifiers: node
            .map(|node| parse_modifiers(node, source))
            .unwrap_or_else(|| vec![Modifier::Public]),
        body: Some(body),
        span: node.map(span).unwrap_or_default(),
    }
}

fn record_component_assignment_stmts(components: &[Param]) -> Vec<Stmt> {
    components
        .iter()
        .map(|component| Stmt::Assign {
            target: Expr::FieldAccess(Box::new(Expr::This), component.name.clone()),
            value: Expr::Var(component.name.clone()),
        })
        .collect()
}

fn record_constructor_is_canonical(ctor: &ConstructorDecl, components: &[Param]) -> bool {
    ctor.params.len() == components.len()
        && ctor
            .params
            .iter()
            .zip(components)
            .all(|(param, component)| param.ty == component.ty)
}

fn parse_enum_member_node(
    node: tree_sitter::Node,
    enum_name: &str,
    source: &str,
    diagnostics: &mut DiagnosticBag,
    members: &mut Vec<Member>,
) {
    match node.kind() {
        "enum_constant" => {
            if let Some(field) = parse_enum_constant_decl(&node, enum_name, source) {
                members.push(Member::Field(field));
            }
        }
        "field_declaration" => members.extend(parse_field_decl(node, source)),
        "constructor_declaration" => {
            members.push(Member::Constructor(parse_constructor_decl(
                node,
                source,
                diagnostics,
            )));
        }
        "method_declaration" => {
            members.push(Member::Method(parse_method_decl(node, source, diagnostics)));
        }
        "class_declaration" => {
            members.push(Member::Type(parse_class_decl(node, source, diagnostics)));
        }
        "interface_declaration" => {
            members.push(Member::Type(parse_interface_decl(
                node,
                source,
                diagnostics,
            )));
        }
        "enum_declaration" => {
            members.push(Member::Type(parse_enum_decl(node, source, diagnostics)));
        }
        "record_declaration" => {
            members.push(Member::Type(parse_record_decl(node, source, diagnostics)));
        }
        "annotation_type_declaration" => {
            members.push(Member::Type(parse_annotation_type_decl(
                node,
                source,
                diagnostics,
            )));
        }
        "enum_body_declarations" => {
            for child in named_children(node) {
                parse_enum_member_node(child, enum_name, source, diagnostics, members);
            }
        }
        _ => {
            if let Some(member) = unsupported_member_for_node(node) {
                members.push(member);
            }
        }
    }
}

fn parse_enum_constant_decl(
    node: &tree_sitter::Node,
    enum_name: &str,
    source: &str,
) -> Option<FieldDecl> {
    let name = field_text(*node, "name", source)
        .or_else(|| first_named_child(*node).map(|child| text(child, source).to_string()))?;
    let args = node
        .child_by_field_name("arguments")
        .or_else(|| {
            named_children(*node)
                .into_iter()
                .find(|child| child.kind() == "argument_list")
        })
        .map(|args| parse_arg_list(args, source))
        .unwrap_or_default();
    let body = named_children(*node)
        .into_iter()
        .find(|child| child.kind() == "class_body")
        .map(|body| parse_anonymous_class_body(body, source));
    let initializer = (!args.is_empty() || body.is_some()).then(|| Expr::New {
        ty: JavaType::Named {
            name: enum_name.to_string(),
            args: Vec::new(),
        },
        qualifier: None,
        args,
        body,
        resolved: None,
    });
    Some(FieldDecl {
        name,
        ty: JavaType::Named {
            name: enum_name.to_string(),
            args: Vec::new(),
        },
        modifiers: vec![Modifier::Public, Modifier::Static, Modifier::Final],
        initializer,
        span: span(*node),
    })
}

fn parse_annotation_type_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let mut members = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "constant_declaration" | "field_declaration" => {
                    members.extend(parse_field_decl(child, source));
                }
                "method_declaration" | "annotation_type_element_declaration" => {
                    members.push(Member::Method(parse_method_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                _ => {
                    if let Some(member) = unsupported_member_for_node(child) {
                        members.push(member);
                    }
                }
            }
        }
    }

    TypeDecl {
        static_init_blocks: Vec::new(),
        kind: TypeKind::Interface,
        name: field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string()),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        modifiers: parse_modifiers(node, source),
        extends: None,
        implements: Vec::new(),
        permits: Vec::new(),
        members,
        span: span(node),
    }
}

fn parse_field_decl(node: tree_sitter::Node, source: &str) -> Vec<Member> {
    let ty = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("field".to_string()));
    let declaration_text = text(node, source);
    let mut modifiers = parse_modifiers(node, source);
    if node.kind() == "constant_declaration" {
        for implicit in [Modifier::Public, Modifier::Static, Modifier::Final] {
            if !modifiers.contains(&implicit) {
                modifiers.push(implicit);
            }
        }
    }
    let mut declarators = field_children(node, "declarator");
    if declarators.is_empty() {
        declarators = named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "variable_declarator")
            .collect();
    }
    declarators
        .into_iter()
        .filter_map(|decl| {
            let name = field_text(decl, "name", source)?;
            let initializer = decl
                .child_by_field_name("value")
                .map(|value| parse_expr_node(value, source));
            Some(Member::Field(FieldDecl {
                name: field_decl_name(&name, &modifiers),
                ty: type_with_declarator_array_dimensions(&ty, declaration_text, &name),
                modifiers: modifiers.clone(),
                initializer,
                span: span(decl),
            }))
        })
        .collect()
}

fn field_decl_name(name: &str, modifiers: &[Modifier]) -> String {
    if modifiers.contains(&Modifier::Static)
        && modifiers.contains(&Modifier::Final)
        && is_java_constant_name(name)
    {
        name.to_string()
    } else {
        to_snake_case_unescaped(name)
    }
}

fn is_java_constant_name(name: &str) -> bool {
    name.chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && name.chars().any(|ch| ch.is_ascii_uppercase())
}

fn parse_constructor_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> ConstructorDecl {
    ConstructorDecl {
        name: field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string()),
        rust_name: None,
        type_params: parse_type_parameter_names(node, source),
        type_param_bounds: parse_type_parameter_bounds(node, source),
        params: node
            .child_by_field_name("parameters")
            .map(|params| parse_params_node(params, source))
            .unwrap_or_default(),
        throws: parse_throws(node, source),
        modifiers: parse_modifiers(node, source),
        body: node
            .child_by_field_name("body")
            .map(|body| parse_block_like(body, source, diagnostics)),
        span: span(node),
    }
}

fn parse_method_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> MethodDecl {
    MethodDecl {
        name: field_text(node, "name", source).unwrap_or_else(|| "unknown".to_string()),
        rust_name: None,
        type_params: parse_type_parameter_names(node, source),
        type_param_bounds: parse_type_parameter_bounds(node, source),
        return_type: node
            .child_by_field_name("type")
            .map(|node| parse_type_node(node, source))
            .unwrap_or(JavaType::Void),
        throws: parse_throws(node, source),
        params: node
            .child_by_field_name("parameters")
            .map(|params| parse_params_node(params, source))
            .unwrap_or_default(),
        modifiers: parse_modifiers(node, source),
        body: node
            .child_by_field_name("body")
            .map(|body| parse_block_like(body, source, diagnostics)),
        span: span(node),
    }
}

fn parse_params_node(node: tree_sitter::Node, source: &str) -> Vec<Param> {
    let params = named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "formal_parameter" | "spread_parameter"))
        .filter_map(|param| {
            let name = field_text(param, "name", source)?;
            let is_varargs =
                param.kind() == "spread_parameter" || text(param, source).contains("...");
            let ty = param
                .child_by_field_name("type")
                .map(|node| {
                    let mut ty = parse_type_node(node, source);
                    if is_varargs {
                        ty = JavaType::Array(Box::new(ty));
                    }
                    ty = type_with_declarator_array_dimensions(&ty, text(param, source), &name);
                    ty
                })
                .or_else(|| {
                    is_varargs
                        .then(|| varargs_type_from_param_text(text(param, source)))
                        .flatten()
                })
                .unwrap_or_else(|| JavaType::Unknown(name.clone()));
            Some(Param {
                name: to_snake_case(&name),
                source_name: to_snake_case_unescaped(&name),
                ty,
                is_varargs,
                binding: None,
            })
        })
        .collect::<Vec<_>>();
    if params.is_empty()
        || (text(node, source).contains("...") && !params.iter().any(|p| p.is_varargs))
    {
        let fallback = parse_params_text_fallback(text(node, source));
        if fallback.len() > params.len() {
            fallback
        } else {
            params
        }
    } else {
        params
    }
}

fn parse_params_text_fallback(params_text: &str) -> Vec<Param> {
    let params_text = params_text
        .trim()
        .strip_prefix('(')
        .and_then(|text| text.strip_suffix(')'))
        .unwrap_or(params_text)
        .trim();
    if params_text.is_empty() {
        return Vec::new();
    }

    params_text
        .split(',')
        .filter_map(parse_param_text_fallback)
        .collect()
}

fn parse_param_text_fallback(param_text: &str) -> Option<Param> {
    let mut param_text = param_text.trim();
    while let Some(rest) = param_text.strip_prefix("final ") {
        param_text = rest.trim_start();
    }
    let (type_text, name) = param_text.rsplit_once(char::is_whitespace)?;
    let mut name = name.trim();
    let mut declarator_array_depth = 0usize;
    while let Some(rest) = name.strip_suffix("[]") {
        declarator_array_depth += 1;
        name = rest.trim_end();
    }
    let is_varargs = type_text.contains("...");
    let mut ty = if is_varargs {
        varargs_type_from_param_text(type_text)?
    } else {
        parse_type_text(type_text)
    };
    for _ in 0..declarator_array_depth.saturating_sub(java_array_depth(&ty)) {
        ty = JavaType::Array(Box::new(ty));
    }
    Some(Param {
        name: to_snake_case(name),
        source_name: to_snake_case_unescaped(name),
        ty,
        is_varargs,
        binding: None,
    })
}

fn varargs_type_from_param_text(param_text: &str) -> Option<JavaType> {
    let type_text = param_text.split("...").next()?.trim();
    (!type_text.is_empty()).then(|| JavaType::Array(Box::new(parse_type_text(type_text))))
}

fn parse_throws(node: tree_sitter::Node, source: &str) -> Vec<JavaType> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == "throws")
        .map(|throws| {
            text(throws, source)
                .trim()
                .trim_start_matches("throws")
                .split(',')
                .filter_map(|part| {
                    let part = part.trim();
                    if part.is_empty() {
                        None
                    } else {
                        Some(parse_type(part))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A `static_initializer` node wraps a single `block`; lower that block's
/// statements. Falls back to the node itself so a malformed tree still yields
/// whatever statements are present rather than silently dropping them.
fn parse_static_initializer_block(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Block {
    let block = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "block")
        .unwrap_or(node);
    parse_block_like(block, source, diagnostics)
}

fn parse_block_like(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Block {
    let mut statements = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            "line_comment" | "block_comment" => {}
            kind if is_empty_statement_kind(kind) => {}
            "statement" => {
                if let Some(stmt) = first_named_child(child) {
                    if !matches!(stmt.kind(), "line_comment" | "block_comment")
                        && !is_empty_statement_kind(stmt.kind())
                    {
                        statements.extend(parse_stmt_nodes(stmt, source, diagnostics));
                    }
                }
            }
            "local_variable_declaration"
            | "expression_statement"
            | "return_statement"
            | "if_statement"
            | "while_statement"
            | "do_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "synchronized_statement"
            | "throw_statement"
            | "assert_statement"
            | "switch_expression"
            | "switch_statement"
            | "break_statement"
            | "continue_statement"
            | "labeled_statement"
            | "block"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "explicit_constructor_invocation" => {
                statements.extend(parse_stmt_nodes(child, source, diagnostics));
            }
            _ if child.is_named() => statements.push(Stmt::Unsupported(format!(
                "unsupported statement node: {}",
                child.kind()
            ))),
            _ => {}
        }
    }
    Block { statements }
}

fn parse_stmt_nodes(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Vec<Stmt> {
    match node.kind() {
        "line_comment" | "block_comment" => Vec::new(),
        "local_variable_declaration" => parse_local_var_decls(node, source),
        _ => vec![parse_stmt_node(node, source, diagnostics)],
    }
}

fn parse_stmt_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    match node.kind() {
        "line_comment" | "block_comment" => Stmt::Block(Block::default()),
        kind if is_empty_statement_kind(kind) => Stmt::Block(Block::default()),
        "block" => Stmt::Block(parse_block_like(node, source, diagnostics)),
        "local_variable_declaration" => parse_local_var_decl(node, source),
        "expression_statement" => first_named_child(node)
            .map(|expr| parse_expr_statement(expr, source))
            .unwrap_or_else(|| Stmt::Unsupported("empty expression statement".to_string())),
        "return_statement" => first_named_child(node)
            .map(|expr| Stmt::Return(Some(parse_expr_node(expr, source))))
            .unwrap_or(Stmt::Return(None)),
        "if_statement" => parse_if_node(node, source, diagnostics),
        "while_statement" => parse_while_node(node, source, diagnostics),
        "do_statement" => parse_do_while_node(node, source, diagnostics),
        "for_statement" => parse_for_node(node, source, diagnostics),
        "enhanced_for_statement" => parse_enhanced_for_node(node, source, diagnostics),
        "labeled_statement" => parse_labeled_node(node, source, diagnostics),
        "break_statement" => {
            Stmt::Break(first_named_child(node).map(|id| parse_label_ident(id, source)))
        }
        "continue_statement" => {
            Stmt::Continue(first_named_child(node).map(|id| parse_label_ident(id, source)))
        }
        "try_statement" | "try_with_resources_statement" => {
            parse_try_node(node, source, diagnostics)
        }
        "synchronized_statement" => parse_synchronized_node(node, source, diagnostics),
        "throw_statement" => first_named_child(node)
            .map(|expr| Stmt::Throw(parse_expr_node(expr, source)))
            .unwrap_or_else(|| Stmt::Unsupported("throw without expression".to_string())),
        "assert_statement" => parse_assert_node(node, source),
        "switch_expression" | "switch_block" | "switch_statement" => {
            parse_switch_node(node, source, diagnostics)
        }
        "class_declaration" => Stmt::LocalType(parse_class_decl(node, source, diagnostics)),
        "interface_declaration" => Stmt::LocalType(parse_interface_decl(node, source, diagnostics)),
        "enum_declaration" => Stmt::LocalType(parse_enum_decl(node, source, diagnostics)),
        "record_declaration" => Stmt::LocalType(parse_record_decl(node, source, diagnostics)),
        "annotation_type_declaration" => {
            Stmt::LocalType(parse_annotation_type_decl(node, source, diagnostics))
        }
        "explicit_constructor_invocation" => {
            parse_constructor_invocation(node, source, diagnostics)
        }
        _ => Stmt::Unsupported(format!("unsupported statement node: {}", node.kind())),
    }
}

fn parse_label_ident(node: tree_sitter::Node, source: &str) -> String {
    to_snake_case(text(node, source))
}

fn parse_assert_node(node: tree_sitter::Node, source: &str) -> Stmt {
    let mut exprs = named_children(node)
        .into_iter()
        .filter(|child| child.kind() != "comment")
        .map(|child| parse_expr_node(child, source));
    let Some(condition) = exprs.next() else {
        return Stmt::Unsupported("assert without condition".to_string());
    };
    let message = exprs.next();
    Stmt::If {
        condition: Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(condition),
        },
        then_block: Block {
            statements: vec![Stmt::Throw(Expr::New {
                ty: JavaType::Named {
                    name: "AssertionError".to_string(),
                    args: Vec::new(),
                },
                qualifier: None,
                args: message.into_iter().collect(),
                body: None,
                resolved: None,
            })],
        },
        else_block: None,
    }
}

fn parse_labeled_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let label = node
        .child_by_field_name("label")
        .or_else(|| {
            named_children(node)
                .into_iter()
                .find(|child| child.kind() == "identifier")
        })
        .map(|label| parse_label_ident(label, source))
        .unwrap_or_else(|| "label".to_string());
    let body = node.child_by_field_name("body").or_else(|| {
        named_children(node)
            .into_iter()
            .find(|child| child.kind() != "identifier")
    });
    let Some(body) = body else {
        return Stmt::Unsupported(format!(
            "labeled statement without body: {}",
            text(node, source)
        ));
    };
    attach_label_to_stmt(label, parse_stmt_node(body, source, diagnostics))
}

fn attach_label_to_stmt(label: String, stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::While {
            condition, body, ..
        } => Stmt::While {
            condition,
            body,
            label: Some(label),
        },
        Stmt::DoWhile {
            body, condition, ..
        } => Stmt::DoWhile {
            body,
            condition,
            label: Some(label),
        },
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => Stmt::For {
            init,
            condition,
            update,
            body,
            label: Some(label),
        },
        Stmt::EnhancedFor {
            var_type,
            var_name,
            source_var_name,
            iterable,
            body,
            binding,
            ..
        } => Stmt::EnhancedFor {
            var_type,
            var_name,
            source_var_name,
            iterable,
            body,
            label: Some(label),
            binding,
        },
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => Stmt::LabeledBlock { label, body },
        other => Stmt::LabeledBlock {
            label,
            body: Block {
                statements: vec![other],
            },
        },
    }
}

fn parse_synchronized_node(
    _node: tree_sitter::Node,
    _source: &str,
    _diagnostics: &mut DiagnosticBag,
) -> Stmt {
    // Java monitors are reentrant and attach to an object. Lowering this as an
    // ordinary block silently drops mutual exclusion; do not claim semantics
    // until the SharedSync/monitor representation exists.
    Stmt::Unsupported("synchronized monitor semantics".to_string())
}

fn parse_try_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let body = node
        .child_by_field_name("body")
        .map(|body| parse_block_like(body, source, diagnostics))
        .unwrap_or_default();
    let resources = node
        .child_by_field_name("resources")
        .map(|resources| parse_try_resources(resources, source))
        .unwrap_or_default();
    let catches = named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "catch_clause")
        .map(|catch| parse_catch_clause(catch, source, diagnostics))
        .collect();
    let finally_block = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "finally_clause")
        .and_then(|finally_clause| {
            named_children(finally_clause)
                .into_iter()
                .find(|child| child.kind() == "block")
        })
        .map(|block| parse_block_like(block, source, diagnostics));
    Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    }
}

fn parse_try_resources(node: tree_sitter::Node, source: &str) -> Vec<Stmt> {
    named_children(node)
        .into_iter()
        .flat_map(|child| match child.kind() {
            "local_variable_declaration" => parse_local_var_decls(child, source),
            "resource" => parse_resource_node(child, source),
            "identifier" | "field_access" => vec![Stmt::Expr(parse_expr_node(child, source))],
            _ => Vec::new(),
        })
        .collect()
}

fn parse_resource_node(node: tree_sitter::Node, source: &str) -> Vec<Stmt> {
    if let Some(stmt) = parse_resource_declaration_text(text(node, source)) {
        return vec![stmt];
    }
    named_children(node)
        .into_iter()
        .flat_map(|child| match child.kind() {
            "local_variable_declaration" => parse_local_var_decls(child, source),
            "identifier" | "field_access" => vec![Stmt::Expr(parse_expr_node(child, source))],
            _ => Vec::new(),
        })
        .collect()
}

fn parse_resource_declaration_text(src: &str) -> Option<Stmt> {
    let (left, right) = src.split_once('=')?;
    let left = left.trim();
    let (ty, name) = left.rsplit_once(char::is_whitespace)?;
    Some(Stmt::VarDecl {
        ty: parse_type(ty.trim()),
        name: to_snake_case(name.trim()),
        source_name: to_snake_case_unescaped(name.trim()),
        initializer: Some(parse_expr(right.trim())),
        binding: None,
    })
}

fn parse_catch_clause(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> CatchClause {
    let parameter = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "catch_formal_parameter");
    let types = parameter
        .and_then(|param| {
            named_children(param)
                .into_iter()
                .find(|child| child.kind() == "catch_type")
        })
        .map(|catch_type| {
            text(catch_type, source)
                .split('|')
                .map(|part| parse_type(part.trim()))
                .collect()
        })
        .unwrap_or_default();
    let (name, source_name) = parameter
        .and_then(|param| field_text(param, "name", source))
        .map(|name| (to_snake_case(&name), to_snake_case_unescaped(&name)))
        .unwrap_or_else(|| ("ex".to_string(), "ex".to_string()));
    let body = node
        .child_by_field_name("body")
        .map(|body| parse_block_like(body, source, diagnostics))
        .unwrap_or_default();
    CatchClause {
        types,
        name,
        source_name,
        body,
        binding: None,
    }
}

fn parse_switch_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let selector = node
        .child_by_field_name("condition")
        .map(|condition| parse_condition_node(condition, source))
        .unwrap_or_else(|| unsupported_expr_node(node, "switch without selector"));
    let cases = node
        .child_by_field_name("body")
        .map(|body| parse_switch_block(body, source, diagnostics))
        .unwrap_or_default();
    Stmt::Switch { selector, cases }
}

fn parse_switch_block(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Vec<SwitchCase> {
    named_children(node)
        .into_iter()
        .filter_map(|child| match child.kind() {
            "switch_block_statement_group" => Some(parse_switch_group(child, source, diagnostics)),
            "switch_rule" => Some(parse_switch_rule_case(child, source, diagnostics)),
            _ => None,
        })
        .collect()
}

fn parse_switch_group(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> SwitchCase {
    let mut labels = Vec::new();
    let mut statements = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            "switch_label" => labels.extend(parse_switch_labels(child, source)),
            "line_comment" | "block_comment" => {}
            "statement" => {
                if let Some(stmt) = first_named_child(child) {
                    if !is_empty_statement_kind(stmt.kind()) {
                        statements.extend(parse_stmt_nodes(stmt, source, diagnostics));
                    }
                }
            }
            _ => statements.extend(parse_stmt_nodes(child, source, diagnostics)),
        }
    }
    SwitchCase { labels, statements }
}

fn parse_switch_rule_case(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> SwitchCase {
    let mut labels = Vec::new();
    let mut statements = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            "switch_label" => labels.extend(parse_switch_labels(child, source)),
            "line_comment" | "block_comment" => {}
            "expression_statement" | "throw_statement" | "block" => {
                statements.extend(parse_stmt_nodes(child, source, diagnostics));
            }
            _ => statements.extend(parse_stmt_nodes(child, source, diagnostics)),
        }
    }
    if !statements
        .last()
        .is_some_and(|stmt| matches!(stmt, Stmt::Break(_) | Stmt::Return(_) | Stmt::Throw(_)))
    {
        statements.push(Stmt::Break(None));
    }
    SwitchCase { labels, statements }
}

fn parse_switch_labels(node: tree_sitter::Node, source: &str) -> Vec<SwitchLabel> {
    let labels = named_children(node)
        .into_iter()
        .filter(|label| !matches!(label.kind(), "line_comment" | "block_comment" | "guard"))
        .map(|label| SwitchLabel::Expr(parse_expr_node(label, source)))
        .collect::<Vec<_>>();
    if labels.is_empty() {
        vec![SwitchLabel::Default]
    } else {
        labels
    }
}

fn parse_constructor_invocation(
    node: tree_sitter::Node,
    source: &str,
    _diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let kind = match node.child_by_field_name("constructor") {
        Some(constructor) if constructor.kind() == "super" => ConstructorInvocationKind::Super,
        _ => ConstructorInvocationKind::This,
    };
    Stmt::ConstructorInvocation {
        kind,
        args: node
            .child_by_field_name("arguments")
            .map(|args| parse_arg_list(args, source))
            .unwrap_or_default(),
        resolved: None,
    }
}

fn parse_expr_statement(node: tree_sitter::Node, source: &str) -> Stmt {
    if node.kind() == "assignment_expression" {
        let target = node
            .child_by_field_name("left")
            .map(|n| parse_expr_node(n, source));
        let value = node
            .child_by_field_name("right")
            .map(|n| parse_expr_node(n, source));
        let op = node
            .child_by_field_name("operator")
            .map(|n| text(n, source));
        if let (Some(target), Some(value)) = (target, value) {
            let value = compound_assignment_value(op, target.clone(), value);
            return Stmt::Assign { target, value };
        }
    }
    Stmt::Expr(parse_expr_node(node, source))
}

fn parse_local_var_decl(node: tree_sitter::Node, source: &str) -> Stmt {
    parse_local_var_decls(node, source)
        .into_iter()
        .next()
        .unwrap_or_else(|| Stmt::Unsupported("local variable without declarator".to_string()))
}

fn parse_local_var_decls(node: tree_sitter::Node, source: &str) -> Vec<Stmt> {
    let ty = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("local".to_string()));
    let declaration_text = text(node, source);
    let declarators = field_children(node, "declarator");
    if declarators.is_empty() {
        return vec![Stmt::Unsupported(
            "local variable without declarator".to_string(),
        )];
    }
    declarators
        .into_iter()
        .map(|declarator| {
            let name =
                field_text(declarator, "name", source).unwrap_or_else(|| "unknown".to_string());
            let initializer = declarator
                .child_by_field_name("value")
                .map(|value| parse_expr_node(value, source));
            Stmt::VarDecl {
                ty: type_with_declarator_array_dimensions(&ty, declaration_text, &name),
                name: to_snake_case(&name),
                source_name: to_snake_case_unescaped(&name),
                initializer,
                binding: None,
            }
        })
        .collect()
}

fn type_with_declarator_array_dimensions(
    base_ty: &JavaType,
    declaration_text: &str,
    name: &str,
) -> JavaType {
    let declared_depth = local_declarator_array_depth(declaration_text, name);
    let parsed_depth = java_array_depth(base_ty);
    if declared_depth <= parsed_depth {
        return base_ty.clone();
    }
    (0..(declared_depth - parsed_depth))
        .fold(base_ty.clone(), |ty, _| JavaType::Array(Box::new(ty)))
}

fn local_declarator_array_depth(declaration_text: &str, name: &str) -> usize {
    let Some(name_start) = find_identifier_occurrence(declaration_text, name) else {
        return 0;
    };
    let prefix = &declaration_text[..name_start];
    let suffix = &declaration_text[name_start + name.len()..];
    count_empty_array_suffixes(prefix) + count_immediate_empty_array_suffixes(suffix)
}

fn find_identifier_occurrence(haystack: &str, ident: &str) -> Option<usize> {
    haystack.match_indices(ident).find_map(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + ident.len()..].chars().next();
        let before_boundary = before.is_none_or(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()));
        let after_boundary = after.is_none_or(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()));
        (before_boundary && after_boundary).then_some(index)
    })
}

fn count_empty_array_suffixes(text: &str) -> usize {
    let mut depth = 0usize;
    let mut count = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            '[' if depth == 0 && chars.peek() == Some(&']') => {
                chars.next();
                count += 1;
            }
            _ => {}
        }
    }
    count
}

fn count_immediate_empty_array_suffixes(mut text: &str) -> usize {
    let mut depth = 0;
    loop {
        text = text.trim_start();
        let Some(rest) = text.strip_prefix("[]") else {
            return depth;
        };
        depth += 1;
        text = rest;
    }
}

fn java_array_depth(ty: &JavaType) -> usize {
    match ty {
        JavaType::Array(inner) => 1 + java_array_depth(inner),
        _ => 0,
    }
}

fn parse_if_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    Stmt::If {
        condition: node
            .child_by_field_name("condition")
            .map(|condition| parse_condition_node(condition, source))
            .unwrap_or_else(|| unsupported_expr_node(node, "if_statement without condition")),
        then_block: node
            .child_by_field_name("consequence")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
        else_block: node
            .child_by_field_name("alternative")
            .map(|stmt| statement_as_block(stmt, source, diagnostics)),
    }
}

fn parse_while_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    Stmt::While {
        condition: node
            .child_by_field_name("condition")
            .map(|condition| parse_condition_node(condition, source))
            .unwrap_or_else(|| unsupported_expr_node(node, "while_statement without condition")),
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
        label: None,
    }
}

fn parse_do_while_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    Stmt::DoWhile {
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
        condition: node
            .child_by_field_name("condition")
            .map(|condition| parse_condition_node(condition, source))
            .unwrap_or_else(|| unsupported_expr_node(node, "do_statement without condition")),
        label: None,
    }
}

fn parse_for_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let init = field_children(node, "init")
        .into_iter()
        .flat_map(|init| {
            if init.kind() == "local_variable_declaration" {
                parse_local_var_decls(init, source)
            } else {
                vec![Stmt::Expr(parse_expr_node(init, source))]
            }
        })
        .collect();
    let update = field_children(node, "update")
        .into_iter()
        .map(|update| parse_expr_node(update, source))
        .collect();
    Stmt::For {
        init,
        condition: node
            .child_by_field_name("condition")
            .map(|condition| parse_expr_node(condition, source)),
        update,
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
        label: None,
    }
}

fn parse_enhanced_for_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let raw_name = node
        .child_by_field_name("name")
        .map(|node| text(node, source).to_string());
    let var_name = raw_name
        .as_deref()
        .map(to_snake_case)
        .unwrap_or_else(|| "item".to_string());
    let source_var_name = raw_name
        .as_deref()
        .map(to_snake_case_unescaped)
        .unwrap_or_else(|| "item".to_string());
    let base_var_type = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("enhanced_for".to_string()));
    let var_type = raw_name
        .as_deref()
        .map(|name| type_with_declarator_array_dimensions(&base_var_type, text(node, source), name))
        .unwrap_or(base_var_type);
    Stmt::EnhancedFor {
        var_type,
        var_name,
        source_var_name,
        iterable: node
            .child_by_field_name("value")
            .map(|node| parse_expr_node(node, source))
            .unwrap_or_else(|| {
                unsupported_expr_node(node, "enhanced_for_statement without iterable")
            }),
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
        label: None,
        binding: None,
    }
}

fn statement_as_block(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Block {
    if node.kind() == "block" {
        parse_block_like(node, source, diagnostics)
    } else if node.kind() == "statement" {
        first_named_child(node)
            .filter(|stmt| !is_empty_statement_kind(stmt.kind()))
            .map(|stmt| Block {
                statements: parse_stmt_nodes(stmt, source, diagnostics),
            })
            .unwrap_or_default()
    } else if is_empty_statement_kind(node.kind()) {
        Block::default()
    } else {
        Block {
            statements: parse_stmt_nodes(node, source, diagnostics),
        }
    }
}

fn is_empty_statement_kind(kind: &str) -> bool {
    matches!(kind, ";" | "empty_statement")
}

fn parse_condition_node(node: tree_sitter::Node, source: &str) -> Expr {
    if node.kind() == "parenthesized_expression" {
        first_named_non_comment_child(node)
            .map(|child| parse_expr_node(child, source))
            .unwrap_or_else(|| unsupported_expr_node(node, "empty parenthesized condition"))
    } else {
        parse_expr_node(node, source)
    }
}

fn parse_expr_node(node: tree_sitter::Node, source: &str) -> Expr {
    match node.kind() {
        "parenthesized_expression" => first_named_non_comment_child(node)
            .map(|child| parse_expr_node(child, source))
            .unwrap_or_else(|| unsupported_expr_node(node, "empty parenthesized expression")),
        "identifier" => Expr::Var(to_snake_case_unescaped(text(node, source))),
        "this" => Expr::This,
        "super" => Expr::Super,
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => parse_integer_literal(text(node, source)),
        "decimal_floating_point_literal" => parse_decimal_floating_literal(text(node, source)),
        "hex_floating_point_literal" => parse_hex_floating_literal(text(node, source)),
        "true" => Expr::Literal(Literal::Bool(true)),
        "false" => Expr::Literal(Literal::Bool(false)),
        "null_literal" => Expr::Literal(Literal::Null),
        "string_literal" => {
            Expr::Literal(Literal::String(parse_string_literal(text(node, source))))
        }
        "character_literal" => Expr::Literal(Literal::Char(parse_char_literal(text(node, source)))),
        "field_access" => {
            let object = node.child_by_field_name("object").map(|n| {
                if n.kind() == "identifier" {
                    let ident = text(n, source);
                    if ident
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_uppercase())
                    {
                        return Expr::Var(ident.to_string());
                    }
                }
                parse_expr_node(n, source)
            });
            let field = field_text(node, "field", source).unwrap_or_else(|| "unknown".to_string());
            if let Some(object) = object {
                Expr::FieldAccess(Box::new(object), field)
            } else {
                Expr::Var(to_snake_case_unescaped(&field))
            }
        }
        "array_access" => {
            let mut children = named_children(node).into_iter();
            let array = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| unsupported_expr_node(node, "array_access without array"));
            let index = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| unsupported_expr_node(node, "array_access without index"));
            Expr::ArrayAccess(Box::new(array), Box::new(index))
        }
        "array_creation_expression" => parse_array_creation_expr(node, source),
        "array_initializer" => {
            Expr::ArrayInitializer(parse_array_initializer_elements(node, source))
        }
        "binary_expression" => {
            let left = node
                .child_by_field_name("left")
                .map(|n| parse_expr_node(n, source));
            let right = node
                .child_by_field_name("right")
                .map(|n| parse_expr_node(n, source));
            let op = node
                .child_by_field_name("operator")
                .and_then(|op| parse_binary_op(text(op, source)));
            match (left, op, right) {
                (Some(left), Some(op), Some(right)) => Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                _ => unsupported_expr_node(node, "malformed binary_expression"),
            }
        }
        "assignment_expression" => {
            let left = node
                .child_by_field_name("left")
                .map(|n| parse_expr_node(n, source));
            let right = node
                .child_by_field_name("right")
                .map(|n| parse_expr_node(n, source));
            let op = node
                .child_by_field_name("operator")
                .map(|op| text(op, source));
            match (left, right) {
                (Some(left), Some(right)) => Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(left.clone()),
                    right: Box::new(compound_assignment_value(op, left, right)),
                },
                _ => unsupported_expr_node(node, "malformed assignment_expression"),
            }
        }
        "update_expression" => {
            let expr = first_named_child(node)
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| {
                    unsupported_expr_node(node, "update_expression without operand")
                });
            let raw = text(node, source);
            let op = if raw.trim_start().starts_with("++") {
                UnaryOp::PreInc
            } else if raw.trim_start().starts_with("--") {
                UnaryOp::PreDec
            } else if raw.contains("--") {
                UnaryOp::PostDec
            } else {
                UnaryOp::PostInc
            };
            Expr::Unary {
                op,
                expr: Box::new(expr),
            }
        }
        "unary_expression" => {
            let expr = first_named_child(node)
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| unsupported_expr_node(node, "unary_expression without operand"));
            let raw = text(node, source).trim_start();
            let op = if raw.starts_with('!') {
                UnaryOp::Not
            } else if raw.starts_with('-') {
                UnaryOp::Neg
            } else if raw.starts_with('+') {
                UnaryOp::Pos
            } else {
                return unsupported_expr_node(node, "unsupported unary_expression operator");
            };
            Expr::Unary {
                op,
                expr: Box::new(expr),
            }
        }
        "ternary_expression" => {
            let mut children = named_children(node).into_iter();
            let condition = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| {
                    unsupported_expr_node(node, "ternary_expression without condition")
                });
            let then_expr = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| {
                    unsupported_expr_node(node, "ternary_expression without then branch")
                });
            let else_expr = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| {
                    unsupported_expr_node(node, "ternary_expression without else branch")
                });
            Expr::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            }
        }
        "switch_expression" => parse_switch_expr_node(node, source),
        "method_invocation" => {
            let receiver = node.child_by_field_name("object").map(|object| {
                let receiver = interface_super_method_owner_expr(node, source)
                    .unwrap_or_else(|| parse_expr_node(object, source));
                Box::new(receiver)
            });
            Expr::Call {
                receiver,
                name: field_text(node, "name", source).unwrap_or_else(|| "unknown".to_string()),
                args: node
                    .child_by_field_name("arguments")
                    .map(|args| parse_arg_list(args, source))
                    .unwrap_or_default(),
                resolved: None,
            }
        }
        "object_creation_expression" => {
            let ty = parse_object_creation_type_node(node, source)
                .unwrap_or_else(|| JavaType::Unknown("new".to_string()));
            let qualifier = parse_object_creation_qualifier_node(node, source).map(Box::new);
            Expr::New {
                ty,
                qualifier,
                args: node
                    .child_by_field_name("arguments")
                    .map(|args| parse_arg_list(args, source))
                    .unwrap_or_default(),
                body: named_children(node)
                    .into_iter()
                    .find(|child| child.kind() == "class_body")
                    .map(|body| parse_anonymous_class_body(body, source)),
                resolved: None,
            }
        }
        "cast_expression" => Expr::Cast {
            ty: parse_cast_type_node(node, source),
            expr: Box::new(
                node.child_by_field_name("value")
                    .map(|value| parse_expr_node(value, source))
                    .unwrap_or_else(|| {
                        unsupported_expr_node(node, "cast_expression without value")
                    }),
            ),
        },
        "instanceof_expression" => parse_instanceof_expr_node(node, source),
        "lambda_expression" => parse_lambda_node(node, source),
        "method_reference" => parse_method_reference_node(node, source),
        "class_literal" => Expr::ClassLiteral(parse_class_literal_type_node(node, source)),
        _ => unsupported_expr_node(
            node,
            &format!("unsupported expression node: {}", node.kind()),
        ),
    }
}

fn interface_super_method_owner_expr(node: tree_sitter::Node, source: &str) -> Option<Expr> {
    let rendered = text(node, source);
    let owner = rendered.split_once(".super.")?.0.trim();
    if owner.is_empty() {
        return None;
    }
    Some(Expr::FieldAccess(
        Box::new(parse_static_owner_expr_text(owner)),
        "super".to_string(),
    ))
}

fn parse_static_owner_expr_text(owner: &str) -> Expr {
    owner
        .split('.')
        .filter(|segment| !segment.is_empty())
        .fold(None, |expr, segment| {
            let segment = if expr.is_none()
                && segment
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
            {
                segment.to_string()
            } else {
                to_snake_case_unescaped(segment)
            };
            Some(match expr {
                Some(expr) => Expr::FieldAccess(Box::new(expr), segment),
                None => Expr::Var(segment),
            })
        })
        .unwrap_or_else(|| Expr::Var(to_snake_case_unescaped(owner)))
}

fn unsupported_expr_node(node: tree_sitter::Node, reason: &str) -> Expr {
    Expr::Unsupported {
        reason: reason.to_string(),
        span: span(node),
    }
}

fn unsupported_expr_reason(reason: impl Into<String>) -> Expr {
    Expr::Unsupported {
        reason: reason.into(),
        span: Span::default(),
    }
}

fn parse_switch_expr_node(node: tree_sitter::Node, source: &str) -> Expr {
    let selector = node
        .child_by_field_name("condition")
        .map(|condition| parse_condition_node(condition, source))
        .unwrap_or_else(|| unsupported_expr_node(node, "switch_expression without selector"));
    let cases = node
        .child_by_field_name("body")
        .map(|body| parse_switch_expr_block(body, source))
        .unwrap_or_default();
    Expr::Switch {
        selector: Box::new(selector),
        cases,
    }
}

fn parse_switch_expr_block(node: tree_sitter::Node, source: &str) -> Vec<SwitchExprCase> {
    let mut cases = Vec::new();
    let mut pending_labels = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            "switch_rule" => {
                if !pending_labels.is_empty() {
                    cases.push(SwitchExprCase {
                        labels: std::mem::take(&mut pending_labels),
                        value: unsupported_expr_node(
                            child,
                            "switch expression labels without value",
                        ),
                    });
                }
                cases.push(parse_switch_expr_rule(child, source));
            }
            "switch_block_statement_group" => {
                let group = parse_switch_expr_yield_group(child, source);
                pending_labels.extend(group.labels);
                if let Some(value) = group.value {
                    cases.push(SwitchExprCase {
                        labels: std::mem::take(&mut pending_labels),
                        value,
                    });
                }
            }
            _ => {}
        }
    }
    if !pending_labels.is_empty() {
        cases.push(SwitchExprCase {
            labels: pending_labels,
            value: unsupported_expr_node(node, "switch expression group without yield"),
        });
    }
    cases
}

fn parse_switch_expr_rule(node: tree_sitter::Node, source: &str) -> SwitchExprCase {
    let mut labels = Vec::new();
    let mut value = None;
    for child in named_children(node) {
        match child.kind() {
            "switch_label" => labels.extend(parse_switch_labels(child, source)),
            "expression_statement" => {
                value = first_named_child(child).map(|expr| parse_expr_node(expr, source));
            }
            "throw_statement" => {
                value = Some(unsupported_expr_node(
                    child,
                    "unsupported switch expression throw arm",
                ));
            }
            "block" => {
                value = Some(unsupported_expr_node(
                    child,
                    "unsupported switch expression block arm",
                ));
            }
            _ => {}
        }
    }
    SwitchExprCase {
        labels,
        value: value
            .unwrap_or_else(|| unsupported_expr_node(node, "switch expression rule without value")),
    }
}

struct ParsedSwitchExprYieldGroup {
    labels: Vec<SwitchLabel>,
    value: Option<Expr>,
}

fn parse_switch_expr_yield_group(
    node: tree_sitter::Node,
    source: &str,
) -> ParsedSwitchExprYieldGroup {
    let mut labels = Vec::new();
    let mut value = None;
    for child in named_children(node) {
        match child.kind() {
            "switch_label" => labels.extend(parse_switch_labels(child, source)),
            "statement" => {
                if let Some(stmt) = first_named_child(child)
                    && stmt.kind() == "yield_statement"
                {
                    value = parse_yield_statement_value(stmt, source);
                }
            }
            "yield_statement" => value = parse_yield_statement_value(child, source),
            "throw_statement" => {
                value = Some(unsupported_expr_node(
                    child,
                    "unsupported switch expression throw arm",
                ));
            }
            "block" => {
                value = Some(unsupported_expr_node(
                    child,
                    "unsupported switch expression block arm",
                ));
            }
            _ => {}
        }
    }
    ParsedSwitchExprYieldGroup { labels, value }
}

fn parse_yield_statement_value(node: tree_sitter::Node, source: &str) -> Option<Expr> {
    first_named_child(node).map(|expr| parse_expr_node(expr, source))
}

fn parse_array_creation_expr(node: tree_sitter::Node, source: &str) -> Expr {
    let element_type = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("array".to_string()));
    let initializer = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "array_initializer")
        .map(|initializer| parse_array_initializer_elements(initializer, source));
    let dimensions = named_children(node)
        .into_iter()
        .filter(|child| child.kind() == "dimensions_expr")
        .flat_map(|child| named_children(child).into_iter())
        .filter(|child| child.kind() != "dimensions")
        .map(|child| parse_expr_node(child, source))
        .collect();
    Expr::ArrayCreation {
        element_type,
        dimensions,
        initializer,
    }
}

fn parse_cast_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    // The `type` field is authoritative. This used to prefer a re-parse of the
    // text between the cast parentheses whenever that produced an array or a
    // generic type, because `parse_array_type_node` dropped a dimension on
    // `int[][]` — a workaround for that bug, not a reason to parse types from
    // text. With the dimension bug fixed the tree and the text agree on every
    // shape the probes cover, so the text path is gone.
    field_children(node, "type")
        .into_iter()
        .next()
        .map(|ty| parse_type_node(ty, source))
        .unwrap_or_else(|| JavaType::Unknown("cast".to_string()))
}

fn parse_array_initializer_elements(node: tree_sitter::Node, source: &str) -> Vec<Expr> {
    named_children(node)
        .into_iter()
        .filter(|child| !matches!(child.kind(), "line_comment" | "block_comment"))
        .map(|child| parse_expr_node(child, source))
        .collect()
}

fn parse_anonymous_class_body(node: tree_sitter::Node, source: &str) -> Vec<Member> {
    let mut members = Vec::new();
    let mut diagnostics = DiagnosticBag::new();
    for child in named_children(node) {
        match child.kind() {
            "field_declaration" => members.extend(parse_field_decl(child, source)),
            "method_declaration" => {
                members.push(Member::Method(parse_method_decl(
                    child,
                    source,
                    &mut diagnostics,
                )));
            }
            "constructor_declaration" => {
                members.push(Member::Constructor(parse_constructor_decl(
                    child,
                    source,
                    &mut diagnostics,
                )));
            }
            "class_declaration" => {
                members.push(Member::Type(parse_class_decl(
                    child,
                    source,
                    &mut diagnostics,
                )));
            }
            "interface_declaration" => {
                members.push(Member::Type(parse_interface_decl(
                    child,
                    source,
                    &mut diagnostics,
                )));
            }
            "record_declaration" => {
                members.push(Member::Type(parse_record_decl(
                    child,
                    source,
                    &mut diagnostics,
                )));
            }
            _ => {
                if let Some(member) = unsupported_member_for_node(child) {
                    members.push(member);
                }
            }
        }
    }
    members
}

fn parse_lambda_node(node: tree_sitter::Node, source: &str) -> Expr {
    let params = node
        .child_by_field_name("parameters")
        .map(|params| parse_lambda_params(params, source))
        .unwrap_or_default();
    let mut diagnostics = DiagnosticBag::new();
    let body = node
        .child_by_field_name("body")
        .map(|body| {
            if body.kind() == "block" {
                LambdaBody::Block(parse_block_like(body, source, &mut diagnostics))
            } else {
                LambdaBody::Expr(Box::new(parse_expr_node(body, source)))
            }
        })
        .unwrap_or_else(|| {
            LambdaBody::Expr(Box::new(unsupported_expr_node(
                node,
                "lambda_expression without body",
            )))
        });
    Expr::Lambda { params, body }
}

fn parse_lambda_params(node: tree_sitter::Node, source: &str) -> Vec<LambdaParam> {
    match node.kind() {
        "identifier" => vec![LambdaParam {
            name: to_snake_case(text(node, source)),
            source_name: to_snake_case_unescaped(text(node, source)),
            ty: None,
        }],
        "inferred_parameters" => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "identifier")
            .map(|id| LambdaParam {
                name: to_snake_case(text(id, source)),
                source_name: to_snake_case_unescaped(text(id, source)),
                ty: None,
            })
            .collect(),
        "formal_parameters" => parse_params_node(node, source)
            .into_iter()
            .map(|param| LambdaParam {
                name: param.name,
                source_name: param.source_name,
                ty: Some(param.ty),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_method_reference_node(node: tree_sitter::Node, source: &str) -> Expr {
    let raw = text(node, source);
    let (target, method) = raw
        .split_once("::")
        .map(|(target, method)| (target.trim().to_string(), method.trim().to_string()))
        .unwrap_or_else(|| (raw.to_string(), "unknown".to_string()));
    Expr::MethodReference { target, method }
}

fn parse_arg_list(node: tree_sitter::Node, source: &str) -> Vec<Expr> {
    named_children(node)
        .into_iter()
        .filter(|arg| !matches!(arg.kind(), "line_comment" | "block_comment"))
        .map(|arg| parse_expr_node(arg, source))
        .collect()
}

pub fn parse_type(src: &str) -> JavaType {
    parse_type_text(src)
}

pub fn parse_hierarchy_type(src: &str) -> JavaType {
    parse_hierarchy_type_text(src)
}

fn parse_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    match node.kind() {
        "void_type" => JavaType::Void,
        "boolean_type" => JavaType::Primitive(PrimitiveType::Boolean),
        "integral_type" | "floating_point_type" => parse_primitive_type_node(node, source)
            .unwrap_or_else(|| parse_type_text(text(node, source))),
        "type_identifier" | "identifier" | "scoped_type_identifier" | "scoped_identifier" => {
            parse_named_type_node(node, source)
        }
        "array_type" => parse_array_type_node(node, source)
            .unwrap_or_else(|| parse_type_text(text(node, source))),
        "generic_type" => parse_generic_type_node(node, source)
            .unwrap_or_else(|| parse_type_text(text(node, source))),
        "wildcard" => parse_wildcard_type_node(node, source)
            .unwrap_or_else(|| parse_type_text(text(node, source))),
        "annotated_type" => parse_annotated_type_node(node, source)
            .unwrap_or_else(|| parse_type_text(text(node, source))),
        // Defensive: every type-level node kind the grammar produces is
        // handled above. Instrumented over the corpus and the type-shape
        // probes at **0 hits**, so this is not a live text path — but it is
        // kept rather than turned into a diagnostic, because the sample here
        // is 248 files and the real corpora are much larger.
        _ => parse_type_text(text(node, source)),
    }
}

fn parse_primitive_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    Some(JavaType::Primitive(match text(node, source).trim() {
        "byte" => PrimitiveType::Byte,
        "short" => PrimitiveType::Short,
        "char" => PrimitiveType::Char,
        "int" => PrimitiveType::Int,
        "long" => PrimitiveType::Long,
        "float" => PrimitiveType::Float,
        "double" => PrimitiveType::Double,
        _ => return None,
    }))
}

fn parse_named_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    let raw = text(node, source);
    if raw.contains('<') {
        // A named-type node whose own text carries type arguments: the
        // `generic_type` handler owns that shape, so this is a defensive path.
        // Measured at 0 hits across the corpus and the type-shape probes.
        return parse_type_text(raw);
    }
    let name = java_type_name_from_parse_tree_node(node, raw);
    if name == "String" {
        JavaType::String
    } else {
        JavaType::Named {
            name,
            args: Vec::new(),
        }
    }
}

fn java_type_name_from_parse_tree_node(node: tree_sitter::Node, raw: &str) -> String {
    if preserve_explicit_java_io_type_name(raw.trim()) {
        return raw.trim().to_string();
    }
    if matches!(node.kind(), "scoped_type_identifier" | "scoped_identifier")
        && explicit_source_package_qualified_type_name(raw)
    {
        return raw.trim().to_string();
    }
    java_type_name_suffix(raw)
}

fn explicit_source_package_qualified_type_name(raw: &str) -> bool {
    let mut parts = raw
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return false;
    };
    if matches!(first, "java" | "javax") {
        return false;
    }
    first
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
        && parts.any(|part| {
            part.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
}

fn parse_array_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    let element = node
        .child_by_field_name("element")
        .or_else(|| node.child_by_field_name("type"))
        .or_else(|| {
            named_children(node).into_iter().find(|child| {
                !matches!(
                    child.kind(),
                    "dimensions" | "annotation" | "marker_annotation"
                )
            })
        })?;
    // `int[][]` is one `array_type` whose `dimensions` child carries *both*
    // pairs of brackets, not two nested `array_type` nodes. Wrapping once
    // dropped a dimension, which is why `parse_cast_type_node` preferred a
    // text re-parse over this result — a workaround for this bug rather than a
    // reason to keep parsing types from text.
    let depth = array_type_node_dimension_depth(node).max(1);
    let mut ty = parse_type_node(element, source);
    for _ in 0..depth {
        ty = JavaType::Array(Box::new(ty));
    }
    Some(ty)
}

/// Number of `[]` pairs directly on an `array_type` node, read from the token
/// stream rather than from the node's source text.
fn array_type_node_dimension_depth(node: tree_sitter::Node) -> usize {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() == "dimensions")
        .map(|dimensions| {
            let mut dim_cursor = dimensions.walk();
            dimensions
                .children(&mut dim_cursor)
                .filter(|token| token.kind() == "[")
                .count()
        })
        .sum()
}

fn parse_generic_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    let base = node.child_by_field_name("type").or_else(|| {
        named_children(node)
            .into_iter()
            .find(|child| child.kind() != "type_arguments")
    })?;
    let args_node = node.child_by_field_name("arguments").or_else(|| {
        named_children(node)
            .into_iter()
            .find(|child| child.kind() == "type_arguments")
    })?;
    let JavaType::Named { name, .. } = parse_type_node(base, source) else {
        return None;
    };
    Some(JavaType::Named {
        name,
        args: parse_type_arguments_node(args_node, source),
    })
}

fn parse_type_arguments_node(node: tree_sitter::Node, source: &str) -> Vec<JavaType> {
    let args: Vec<JavaType> = named_children(node)
        .into_iter()
        .filter(|child| {
            !matches!(
                child.kind(),
                "line_comment" | "block_comment" | "annotation" | "marker_annotation"
            )
        })
        .map(|child| parse_type_node(child, source))
        .collect();
    if !args.is_empty() {
        return args;
    }
    let raw = text(node, source).trim();
    let inner = raw
        .strip_prefix('<')
        .and_then(|raw| raw.strip_suffix('>'))
        .unwrap_or(raw)
        .trim();
    split_top_level_commas(inner)
        .into_iter()
        .map(|part| parse_type_text(part.trim()))
        .collect()
}

fn parse_wildcard_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    let raw = text(node, source).trim();
    if raw == "?" {
        return Some(JavaType::Wildcard(None));
    }
    let bound = node
        .child_by_field_name("bound")
        .or_else(|| named_children(node).into_iter().last())?;
    if raw.contains(" extends ") {
        Some(JavaType::Wildcard(Some(WildcardBound::Extends(Box::new(
            parse_type_node(bound, source),
        )))))
    } else if raw.contains(" super ") {
        Some(JavaType::Wildcard(Some(WildcardBound::Super(Box::new(
            parse_type_node(bound, source),
        )))))
    } else {
        None
    }
}

fn parse_annotated_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    named_children(node)
        .into_iter()
        .rev()
        .find(|child| !matches!(child.kind(), "annotation" | "marker_annotation"))
        .map(|child| parse_type_node(child, source))
}

fn parse_instanceof_expr_node(node: tree_sitter::Node, source: &str) -> Expr {
    Expr::InstanceOf {
        expr: Box::new(
            node.child_by_field_name("left")
                .map(|left| parse_expr_node(left, source))
                .unwrap_or_else(|| {
                    unsupported_expr_node(node, "instanceof_expression without left operand")
                }),
        ),
        ty: node
            .child_by_field_name("right")
            .map(|right| parse_type_node(right, source))
            .unwrap_or_else(|| JavaType::Unknown("instanceof".to_string())),
        binding: node
            .child_by_field_name("name")
            .map(|name| text(name, source).to_string()),
    }
}

fn parse_class_literal_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    let raw = text(node, source).trim();
    if let Some(raw_ty) = raw.strip_suffix(".class") {
        let raw_ty = raw_ty.trim();
        if explicit_source_package_qualified_type_name(raw_ty) {
            return parse_new_type_text(raw_ty);
        }
    }
    first_named_child(node)
        .map(|ty| parse_type_node(ty, source))
        .unwrap_or_else(|| JavaType::Unknown("class".to_string()))
}

fn parse_new_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    let raw = text(node, source).trim();
    parse_new_type_text(raw)
}

fn parse_new_type_text(raw: &str) -> JavaType {
    let mut ty = parse_type_text(raw);
    if type_text_starts_with_type_like_segment(raw)
        || explicit_source_package_qualified_type_name(raw)
    {
        if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &mut ty {
            *name = raw.to_string();
        }
    }
    ty
}

fn parse_object_creation_type_node(node: tree_sitter::Node, source: &str) -> Option<JavaType> {
    let parsed_child = node
        .child_by_field_name("type")
        .map(|node| parse_new_type_node(node, source));
    let args = node.child_by_field_name("arguments")?;
    let prefix = source.get(node.start_byte()..args.start_byte())?;
    let raw_prefix = prefix.trim();
    let raw = raw_prefix
        .strip_prefix("new")
        .or_else(|| raw_prefix.split_once(".new").map(|(_, ty)| ty))
        .map(str::trim)?;
    if raw.is_empty() {
        return parsed_child;
    }
    let raw_ty = parse_new_type_text(raw);
    match (&parsed_child, &raw_ty) {
        (
            Some(JavaType::Named { name: parsed, .. } | JavaType::Unknown(parsed)),
            JavaType::Named { name: raw, .. } | JavaType::Unknown(raw),
        ) if raw.len() > parsed.len() && raw.starts_with(parsed) => Some(raw_ty),
        (None, _) => Some(raw_ty),
        _ => parsed_child,
    }
}

fn parse_object_creation_qualifier_node(node: tree_sitter::Node, source: &str) -> Option<Expr> {
    let args = node.child_by_field_name("arguments")?;
    let prefix = source.get(node.start_byte()..args.start_byte())?.trim();
    let (qualifier, _) = prefix.split_once(".new")?;
    let qualifier = qualifier.trim();
    (!qualifier.is_empty()).then(|| parse_expr(qualifier))
}

fn type_text_starts_with_type_like_segment(raw: &str) -> bool {
    raw.split(['.', '<', '['])
        .next()
        .and_then(|segment| segment.trim().chars().next())
        .is_some_and(|ch| ch.is_ascii_uppercase())
        && raw.contains('.')
        && !raw.contains('<')
}

fn parse_hierarchy_type_text(src: &str) -> JavaType {
    let mut src = src.trim();
    while let Some(rest) = src.strip_prefix("final ") {
        src = rest.trim_start();
    }
    src = src
        .trim_start_matches("extends")
        .trim_start_matches("implements")
        .trim();
    if let Some(inner) = src.strip_suffix("[]") {
        return JavaType::Array(Box::new(parse_hierarchy_type_text(inner)));
    }
    if let Some(open) = src.find('<') {
        let close = matching_angle_close(src, open).unwrap_or(src.len());
        let name = src[..open].trim().to_string();
        let inner = src[open + 1..close].trim();
        return JavaType::Named {
            name,
            args: split_top_level_commas(inner)
                .into_iter()
                .map(|part| parse_type_text(part.trim()))
                .collect(),
        };
    }
    parse_type_text(src)
}

/// Parses a type from **text**.
///
/// Legitimate only for genuinely text-sourced types — `javalibs` signatures and
/// external pack manifests. Source types come from the parse tree via
/// [`parse_type_node`]; see
/// `claude_audits/0022-types-are-reparsed-from-source-text.md`.
fn parse_type_text(src: &str) -> JavaType {
    let mut src = src.trim();
    while let Some(rest) = src.strip_prefix("final ") {
        src = rest.trim_start();
    }
    if let Some(inner) = src.strip_suffix("[]") {
        return JavaType::Array(Box::new(parse_type_text(inner)));
    }
    match src {
        "void" => JavaType::Void,
        "boolean" => JavaType::Primitive(PrimitiveType::Boolean),
        "byte" => JavaType::Primitive(PrimitiveType::Byte),
        "short" => JavaType::Primitive(PrimitiveType::Short),
        "char" => JavaType::Primitive(PrimitiveType::Char),
        "int" => JavaType::Primitive(PrimitiveType::Int),
        "long" => JavaType::Primitive(PrimitiveType::Long),
        "float" => JavaType::Primitive(PrimitiveType::Float),
        "double" => JavaType::Primitive(PrimitiveType::Double),
        "String" => JavaType::String,
        "?" => JavaType::Wildcard(None),
        _ => {
            if let Some(rest) = src.strip_prefix("? extends ") {
                return JavaType::Wildcard(Some(WildcardBound::Extends(Box::new(
                    parse_type_text(rest),
                ))));
            }
            if let Some(rest) = src.strip_prefix("? super ") {
                return JavaType::Wildcard(Some(WildcardBound::Super(Box::new(parse_type_text(
                    rest,
                )))));
            }
            if let Some(open) = src.find('<') {
                if let Some(close) = matching_angle_close(src, open)
                    && let Some(inner_suffix) = src
                        .get(close + 1..)
                        .and_then(|suffix| suffix.strip_prefix('.'))
                    && !inner_suffix.trim().is_empty()
                {
                    return parse_type_text(inner_suffix.trim());
                }
                let name = java_type_name_suffix(src[..open].trim());
                let inner = src[open + 1..src.rfind('>').unwrap_or(src.len())].trim();
                return JavaType::Named {
                    name,
                    args: split_top_level_commas(inner)
                        .into_iter()
                        .map(|part| parse_type_text(part.trim()))
                        .collect(),
                };
            }
            JavaType::Named {
                name: java_type_name_suffix(src),
                args: Vec::new(),
            }
        }
    }
}

fn java_type_name_suffix(src: &str) -> String {
    let src = src.trim();
    if preserve_explicit_java_io_type_name(src) {
        return src.to_string();
    }
    let mut suffix = Vec::new();
    for part in src.split('.') {
        let part = part.trim();
        if part
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            suffix.push(part);
        } else if !suffix.is_empty() {
            suffix.push(part);
        }
    }
    if suffix.is_empty() {
        src.rsplit('.').next().unwrap_or(src).trim().to_string()
    } else {
        suffix.join(".")
    }
}

fn preserve_explicit_java_io_type_name(src: &str) -> bool {
    matches!(
        src,
        "java.io.InputStream"
            | "java.io.OutputStream"
            | "java.io.FilterInputStream"
            | "java.io.FilterOutputStream"
            | "java.io.FileInputStream"
            | "java.io.FileOutputStream"
            | "java.io.BufferedInputStream"
            | "java.io.BufferedOutputStream"
            | "java.io.ByteArrayInputStream"
            | "java.io.ByteArrayOutputStream"
            | "java.io.DataInputStream"
            | "java.io.ObjectInputStream"
            | "java.io.ObjectOutputStream"
            | "java.io.PushbackInputStream"
            | "java.io.SequenceInputStream"
    )
}

/// Expression-only helper used by unit tests and small resolver tests.
/// Normal source parsing flows through tree-sitter `parse_java_source`.
pub fn parse_expr(src: &str) -> Expr {
    let wrapped = format!("class __Expr {{ void __m() {{ Object __x = {src}; }} }}");
    let parsed = parse_java_source(&wrapped);
    parsed
        .program
        .types
        .first()
        .and_then(|ty| {
            ty.members.iter().find_map(|member| match member {
                Member::Method(method) => method.body.as_ref(),
                _ => None,
            })
        })
        .and_then(|body| body.statements.first())
        .and_then(|stmt| match stmt {
            Stmt::VarDecl {
                initializer: Some(expr),
                ..
            } => Some(expr.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            unsupported_expr_reason(format!(
                "unsupported Java expression parse helper input: {src}"
            ))
        })
}

fn collect_syntax_diagnostics(node: tree_sitter::Node, diagnostics: &mut DiagnosticBag) {
    if node.is_error() {
        diagnostics.push(
            crate::diagnostics::Severity::Error,
            "java_syntax_error",
            format!("Java syntax error near {}", node.kind()),
            Some(span(node)),
        );
    } else if node.is_missing() {
        diagnostics.push(
            crate::diagnostics::Severity::Error,
            "java_syntax_missing",
            format!("Java syntax error: missing {}", node.kind()),
            Some(span(node)),
        );
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_diagnostics(child, diagnostics);
    }
}

fn parse_modifiers(node: tree_sitter::Node, source: &str) -> Vec<Modifier> {
    let Some(modifiers) = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "modifiers")
    else {
        return Vec::new();
    };
    let raw = text(modifiers, source);
    let mut out = Vec::new();
    for token in raw.split_whitespace() {
        match token {
            "public" => out.push(Modifier::Public),
            "protected" => out.push(Modifier::Protected),
            "private" => out.push(Modifier::Private),
            "static" => out.push(Modifier::Static),
            "synchronized" => out.push(Modifier::Synchronized),
            "volatile" => out.push(Modifier::Volatile),
            "native" => out.push(Modifier::Native),
            "final" => out.push(Modifier::Final),
            "sealed" => out.push(Modifier::Sealed),
            "abstract" => out.push(Modifier::Abstract),
            _ => {}
        }
    }
    out
}

fn parse_integer_literal(src: &str) -> Expr {
    let src = src.replace('_', "");
    if let Some(num) = src.strip_suffix('L').or_else(|| src.strip_suffix('l')) {
        parse_java_integer_digits_i64(num, true)
            .map(Literal::Long)
            .map(Expr::Literal)
            .unwrap_or_else(|_| {
                unsupported_expr_reason(format!("invalid Java integer literal: {src}"))
            })
    } else {
        match parse_java_integer_digits_i64(&src, false) {
            Ok(value) => match i32::try_from(value) {
                Ok(value) => Expr::Literal(Literal::Int(value)),
                Err(_) if integer_literal_allows_twos_complement_int(&src) => {
                    parse_java_integer_digits_u32(&src)
                        .map(|value| Expr::Literal(Literal::Int(value as i32)))
                        .unwrap_or_else(|_| {
                            unsupported_expr_reason(format!("invalid Java integer literal: {src}"))
                        })
                }
                Err(_) => unsupported_expr_reason(format!("invalid Java integer literal: {src}")),
            },
            Err(_) => unsupported_expr_reason(format!("invalid Java integer literal: {src}")),
        }
    }
}

fn integer_literal_allows_twos_complement_int(src: &str) -> bool {
    src.starts_with("0x")
        || src.starts_with("0X")
        || src.starts_with("0b")
        || src.starts_with("0B")
        || (src.len() > 1 && src.starts_with('0'))
}

fn parse_java_integer_digits_u32(src: &str) -> Result<u32, std::num::ParseIntError> {
    let (digits, radix) =
        if let Some(rest) = src.strip_prefix("0x").or_else(|| src.strip_prefix("0X")) {
            (rest, 16)
        } else if let Some(rest) = src.strip_prefix("0b").or_else(|| src.strip_prefix("0B")) {
            (rest, 2)
        } else if src.len() > 1 && src.starts_with('0') {
            (&src[1..], 8)
        } else {
            (src, 10)
        };
    u32::from_str_radix(digits, radix)
}

fn parse_java_integer_digits_i64(
    src: &str,
    allow_twos_complement_long: bool,
) -> Result<i64, std::num::ParseIntError> {
    let (digits, radix) =
        if let Some(rest) = src.strip_prefix("0x").or_else(|| src.strip_prefix("0X")) {
            (rest, 16)
        } else if let Some(rest) = src.strip_prefix("0b").or_else(|| src.strip_prefix("0B")) {
            (rest, 2)
        } else if src.len() > 1 && src.starts_with('0') {
            (&src[1..], 8)
        } else {
            (src, 10)
        };
    match i64::from_str_radix(digits, radix) {
        Ok(value) => Ok(value),
        Err(err) if allow_twos_complement_long && radix != 10 => u64::from_str_radix(digits, radix)
            .map(|value| value as i64)
            .map_err(|_| err),
        Err(err) => Err(err),
    }
}

fn parse_decimal_floating_literal(src: &str) -> Expr {
    let src = src.replace('_', "");
    if let Some(num) = src.strip_suffix('f').or_else(|| src.strip_suffix('F')) {
        num.parse()
            .map(Literal::Float)
            .map(Expr::Literal)
            .unwrap_or_else(|_| {
                unsupported_expr_reason(format!("invalid Java floating-point literal: {src}"))
            })
    } else {
        let num = src
            .strip_suffix('d')
            .or_else(|| src.strip_suffix('D'))
            .unwrap_or(&src);
        num.parse()
            .map(Literal::Double)
            .map(Expr::Literal)
            .unwrap_or_else(|_| {
                unsupported_expr_reason(format!("invalid Java floating-point literal: {src}"))
            })
    }
}

fn parse_hex_floating_literal(src: &str) -> Expr {
    let src = src.replace('_', "");
    let (body, is_float) = match src.chars().last() {
        Some('f') | Some('F') => (&src[..src.len() - 1], true),
        Some('d') | Some('D') => (&src[..src.len() - 1], false),
        _ => (src.as_str(), false),
    };
    let Some((mantissa, exponent)) = body.split_once(['p', 'P']) else {
        return unsupported_expr_reason(format!("invalid Java hex floating-point literal: {src}"));
    };
    let Some(mantissa) = mantissa
        .strip_prefix("0x")
        .or_else(|| mantissa.strip_prefix("0X"))
    else {
        return unsupported_expr_reason(format!("invalid Java hex floating-point literal: {src}"));
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return unsupported_expr_reason(format!("invalid Java hex floating-point literal: {src}"));
    };
    let Some(value) = parse_hex_float_mantissa(mantissa) else {
        return unsupported_expr_reason(format!("invalid Java hex floating-point literal: {src}"));
    };
    let value = value * 2f64.powi(exponent);
    if is_float {
        Expr::Literal(Literal::Float(value as f32))
    } else {
        Expr::Literal(Literal::Double(value))
    }
}

fn parse_hex_float_mantissa(mantissa: &str) -> Option<f64> {
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut value = 0f64;
    for ch in whole.chars() {
        value = value * 16f64 + ch.to_digit(16)? as f64;
    }
    let mut place = 1f64 / 16f64;
    for ch in fraction.chars() {
        value += ch.to_digit(16)? as f64 * place;
        place /= 16f64;
    }
    Some(value)
}

fn parse_string_literal(src: &str) -> String {
    let src = src.trim();
    if src.starts_with("\"\"\"") && src.ends_with("\"\"\"") && src.len() >= 6 {
        parse_text_block_literal(src)
    } else {
        parse_java_string_escapes(src.trim_start_matches('"').trim_end_matches('"'))
    }
}

fn parse_text_block_literal(src: &str) -> String {
    let raw = &src[3..src.len() - 3];
    let mut content = raw.replace("\r\n", "\n").replace('\r', "\n");
    if let Some(rest) = content.strip_prefix('\n') {
        content = rest.to_string();
    }

    let mut lines = content
        .split('\n')
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let incidental_indent = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let is_closing_delimiter_line = index + 1 == lines.len();
            if !is_closing_delimiter_line && line.trim().is_empty() {
                None
            } else {
                Some(java_whitespace_indent(line))
            }
        })
        .min()
        .unwrap_or(0);

    for line in &mut lines {
        *line = strip_java_whitespace_indent(line, incidental_indent)
            .trim_end_matches(java_text_block_trailing_whitespace)
            .to_string();
    }

    parse_java_string_escapes(&lines.join("\n"))
}

fn java_whitespace_indent(line: &str) -> usize {
    line.chars()
        .take_while(|ch| java_text_block_trailing_whitespace(*ch))
        .count()
}

fn strip_java_whitespace_indent(line: &str, indent: usize) -> &str {
    let mut byte_index = 0usize;
    for (count, (index, ch)) in line.char_indices().enumerate() {
        if count >= indent || !java_text_block_trailing_whitespace(ch) {
            byte_index = index;
            return &line[byte_index..];
        }
        byte_index = index + ch.len_utf8();
    }
    if line.chars().count() >= indent {
        ""
    } else {
        &line[byte_index..]
    }
}

fn java_text_block_trailing_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\u{000c}')
}

fn parse_java_string_escapes(src: &str) -> String {
    let mut out = String::new();
    let mut chars = src.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\n') => {}
            Some('\r') => {
                if matches!(chars.peek(), Some('\n')) {
                    chars.next();
                }
            }
            Some('b') => out.push('\u{0008}'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('f') => out.push('\u{000c}'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            Some('s') => out.push(' '),
            Some(other @ '0'..='7') => {
                let mut digits = String::from(other);
                while digits.len() < 3 && matches!(chars.peek(), Some('0'..='7')) {
                    digits.push(chars.next().unwrap());
                }
                if let Ok(value) = u32::from_str_radix(&digits, 8) {
                    if let Some(ch) = char::from_u32(value) {
                        out.push(ch);
                    }
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn parse_char_literal(src: &str) -> char {
    let inner = src.trim().trim_start_matches('\'').trim_end_matches('\'');
    match inner {
        "\\n" => '\n',
        "\\r" => '\r',
        "\\t" => '\t',
        "\\0" => '\0',
        "\\'" => '\'',
        "\\\"" => '"',
        "\\\\" => '\\',
        _ => inner.chars().next().unwrap_or('\0'),
    }
}

fn compound_assignment_value(op: Option<&str>, target: Expr, value: Expr) -> Expr {
    let op = match op {
        Some("+=") => BinaryOp::Add,
        Some("-=") => BinaryOp::Sub,
        Some("*=") => BinaryOp::Mul,
        Some("/=") => BinaryOp::Div,
        Some("%=") => BinaryOp::Rem,
        Some("&=") => BinaryOp::BitAnd,
        Some("|=") => BinaryOp::BitOr,
        Some("^=") => BinaryOp::BitXor,
        Some("<<=") => BinaryOp::Shl,
        Some(">>=") => BinaryOp::Shr,
        Some(">>>=") => BinaryOp::UShr,
        _ => return value,
    };
    Expr::Binary {
        op,
        left: Box::new(target),
        right: Box::new(value),
    }
}

fn parse_binary_op(src: &str) -> Option<BinaryOp> {
    Some(match src {
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "%" => BinaryOp::Rem,
        "&" => BinaryOp::BitAnd,
        "|" => BinaryOp::BitOr,
        "^" => BinaryOp::BitXor,
        "<<" => BinaryOp::Shl,
        ">>" => BinaryOp::Shr,
        ">>>" => BinaryOp::UShr,
        "==" => BinaryOp::Eq,
        "!=" => BinaryOp::Ne,
        "<" => BinaryOp::Lt,
        "<=" => BinaryOp::Le,
        ">" => BinaryOp::Gt,
        ">=" => BinaryOp::Ge,
        "&&" => BinaryOp::And,
        "||" => BinaryOp::Or,
        _ => return None,
    })
}

fn type_name_text(node: tree_sitter::Node, source: &str) -> String {
    text(node, source)
        .trim()
        .trim_start_matches("extends")
        .trim_start_matches("implements")
        .trim()
        .to_string()
}

fn parse_type_name_list(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect_type_names(node, source, &mut out);
    out
}

fn parse_permits(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let raw = text(node, source);
    let Some((_, rest)) = raw.split_once(" permits ") else {
        return Vec::new();
    };
    let header = rest
        .split_once('{')
        .map(|(header, _)| header)
        .unwrap_or(rest)
        .trim();
    header
        .split(',')
        .map(|name| {
            name.split_whitespace()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn parse_type_parameter_names(node: tree_sitter::Node, source: &str) -> Vec<String> {
    type_parameter_nodes(node)
        .into_iter()
        .filter_map(|param| type_parameter_name(param, source))
        .collect()
}

fn parse_type_parameter_bounds(node: tree_sitter::Node, source: &str) -> Vec<Option<JavaType>> {
    type_parameter_nodes(node)
        .into_iter()
        .map(|param| type_parameter_bound(param, source))
        .collect()
}

/// The `type_parameter` children of a declaration's `type_parameters` node.
///
/// Reading the tree rather than re-parsing the declaration text keeps nested
/// generics, annotations and intersection bounds intact — text scanning had to
/// rediscover all of that with `matching_angle_close` and comma splitting.
fn type_parameter_nodes(node: tree_sitter::Node) -> Vec<tree_sitter::Node> {
    let type_parameters = if node.kind() == "type_parameters" {
        Some(node)
    } else {
        named_children(node)
            .into_iter()
            .find(|child| child.kind() == "type_parameters")
    };
    let Some(type_parameters) = type_parameters else {
        return Vec::new();
    };
    named_children(type_parameters)
        .into_iter()
        .filter(|child| child.kind() == "type_parameter")
        .collect()
}

fn type_parameter_name(param: tree_sitter::Node, source: &str) -> Option<String> {
    named_children(param)
        .into_iter()
        .find(|child| child.kind() == "type_identifier" || child.kind() == "identifier")
        .map(|name| text(name, source).trim().to_string())
        .filter(|name| !name.is_empty())
}

/// The first type of a parameter's `type_bound`.
///
/// An intersection bound (`T extends A & B`) keeps only `A`, matching what the
/// downstream shape model can currently express; the tree makes the rest
/// available when that changes.
fn type_parameter_bound(param: tree_sitter::Node, source: &str) -> Option<JavaType> {
    let bound = named_children(param)
        .into_iter()
        .find(|child| child.kind() == "type_bound")?;
    named_children(bound)
        .into_iter()
        .next()
        .map(|ty| parse_type_node(ty, source))
}

fn matching_angle_close(src: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in src[open..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_type_names(node: tree_sitter::Node, source: &str, out: &mut Vec<String>) {
    match node.kind() {
        "type_identifier" | "scoped_type_identifier" | "generic_type" | "identifier" => {
            out.push(type_name_text(node, source));
        }
        _ => {
            for child in named_children(node) {
                collect_type_names(child, source, out);
            }
        }
    }
}

fn named_children<'a>(node: tree_sitter::Node<'a>) -> Vec<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn field_children<'a>(node: tree_sitter::Node<'a>, field: &str) -> Vec<tree_sitter::Node<'a>> {
    let mut out = Vec::new();
    for idx in 0..node.child_count() {
        let Some(child) = node.child(idx as u32) else {
            continue;
        };
        if node.field_name_for_child(idx as u32) == Some(field) {
            out.push(child);
        }
    }
    out
}

fn first_named_child<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    named_children(node).into_iter().next()
}

fn first_named_non_comment_child<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    named_children(node)
        .into_iter()
        .find(|child| !matches!(child.kind(), "line_comment" | "block_comment"))
}

fn field_text(node: tree_sitter::Node, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|node| text(node, source).to_string())
}

fn text<'a>(node: tree_sitter::Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn span(node: tree_sitter::Node) -> Span {
    Span::new(node.start_byte(), node.end_byte())
}

fn split_top_level_commas(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (idx, ch) in src.char_indices() {
        match ch {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&src[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if !src[start..].trim().is_empty() {
        parts.push(&src[start..]);
    }
    parts
}

pub(crate) fn to_snake_case(name: &str) -> String {
    let out = to_snake_case_unescaped(name);
    if crate::java_naming::rust_keyword_needs_rename(&out) {
        crate::java_naming::rename_non_rawable_rust_keyword(&out)
    } else {
        out
    }
}

pub(crate) fn to_snake_case_unescaped(name: &str) -> String {
    let name = name.trim().trim_start_matches("this.");
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch == '.' {
            out.push('.');
        } else if ch.is_ascii_uppercase() {
            if idx > 0 && !out.ends_with('_') && !out.ends_with('.') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn type_suffix(ty: &JavaType, is_varargs: bool) -> String {
    let mut suffix = match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(p) => format!("{p:?}").to_ascii_lowercase(),
        JavaType::String => "string".to_string(),
        JavaType::Array(inner) => format!("{}_array", type_suffix(inner, false)),
        JavaType::Named { name, .. } => to_snake_case(&java_type_suffix_name(name)),
        JavaType::Wildcard(_) => "wildcard".to_string(),
        JavaType::Unknown(raw) => to_snake_case(&java_type_suffix_name(raw)),
    };
    if is_varargs {
        suffix.push_str("_varargs");
    }
    suffix
}

fn java_type_suffix_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '.' | '$' | '<' | '>' | ',' | '?' | ' ' | '\t' | '\n' | '\r' => '_',
            _ => ch,
        })
        .collect()
}
