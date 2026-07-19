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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => qualify_java_sql_block(block, imported),
    }
}

fn qualify_java_sql_expr(expr: &mut Expr, imported: &JavaSqlImports) {
    match expr {
        Expr::Literal(_) | Expr::Var(_) | Expr::This | Expr::Super | Expr::Raw(_) => {}
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
        Expr::Cast { ty, expr } | Expr::InstanceOf { expr, ty } => {
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
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "field_declaration" => members.extend(parse_field_decl(child, source)),
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
                _ => {}
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
                _ => {}
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
    let mut has_constructor = false;

    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            match child.kind() {
                "field_declaration" => members.extend(parse_field_decl(child, source)),
                "constructor_declaration" => {
                    has_constructor = true;
                    members.push(Member::Constructor(parse_constructor_decl(
                        child,
                        source,
                        diagnostics,
                    )));
                }
                "compact_constructor_declaration" => {
                    has_constructor = true;
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
                _ => {}
            }
        }
    }
    if !has_constructor {
        members.push(Member::Constructor(record_canonical_constructor_decl(
            &name,
            &components,
            None,
            source,
            diagnostics,
        )));
    }

    TypeDecl {
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
        body: None,
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
        body: node
            .and_then(|node| node.child_by_field_name("body"))
            .map(|body| parse_block_like(body, source, diagnostics)),
        span: node.map(span).unwrap_or_default(),
    }
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
        _ => {}
    }
}

fn parse_enum_constant_decl(
    node: &tree_sitter::Node,
    enum_name: &str,
    source: &str,
) -> Option<FieldDecl> {
    let name = field_text(*node, "name", source)
        .or_else(|| first_named_child(*node).map(|child| text(child, source).to_string()))?;
    Some(FieldDecl {
        name: to_snake_case(&name),
        ty: JavaType::Named {
            name: enum_name.to_string(),
            args: Vec::new(),
        },
        modifiers: vec![Modifier::Public, Modifier::Static, Modifier::Final],
        initializer: None,
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
                _ => {}
            }
        }
    }

    TypeDecl {
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
                ty: ty.clone(),
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
        to_snake_case(name)
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
                ty,
                is_varargs,
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
    let is_varargs = type_text.contains("...");
    let ty = if is_varargs {
        varargs_type_from_param_text(type_text)?
    } else {
        parse_type_text(type_text)
    };
    Some(Param {
        name: to_snake_case(name.trim()),
        ty,
        is_varargs,
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

fn parse_block_like(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Block {
    let mut statements = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            kind if is_empty_statement_kind(kind) => {}
            "statement" => {
                if let Some(stmt) = first_named_child(child) {
                    if !is_empty_statement_kind(stmt.kind()) {
                        statements.extend(parse_stmt_nodes(stmt, source, diagnostics));
                    }
                }
            }
            "local_variable_declaration"
            | "expression_statement"
            | "return_statement"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "throw_statement"
            | "switch_expression"
            | "switch_statement"
            | "break_statement"
            | "continue_statement"
            | "block"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "explicit_constructor_invocation" => {
                statements.extend(parse_stmt_nodes(child, source, diagnostics));
            }
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
        "local_variable_declaration" => parse_local_var_decls(node, source),
        _ => vec![parse_stmt_node(node, source, diagnostics)],
    }
}

fn parse_stmt_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    match node.kind() {
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
        "for_statement" => parse_for_node(node, source, diagnostics),
        "enhanced_for_statement" => parse_enhanced_for_node(node, source, diagnostics),
        "break_statement" => {
            Stmt::Break(first_named_child(node).map(|id| text(id, source).to_string()))
        }
        "continue_statement" => {
            Stmt::Continue(first_named_child(node).map(|id| text(id, source).to_string()))
        }
        "try_statement" | "try_with_resources_statement" => {
            parse_try_node(node, source, diagnostics)
        }
        "throw_statement" => first_named_child(node)
            .map(|expr| Stmt::Throw(parse_expr_node(expr, source)))
            .unwrap_or_else(|| Stmt::Unsupported("throw without expression".to_string())),
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
        initializer: Some(parse_expr(right.trim())),
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
    let name = parameter
        .and_then(|param| field_text(param, "name", source))
        .map(|name| to_snake_case(&name))
        .unwrap_or_else(|| "ex".to_string());
    let body = node
        .child_by_field_name("body")
        .map(|body| parse_block_like(body, source, diagnostics))
        .unwrap_or_default();
    CatchClause { types, name, body }
}

fn parse_switch_node(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let selector = node
        .child_by_field_name("condition")
        .map(|condition| parse_condition_node(condition, source))
        .unwrap_or_else(|| Expr::Raw("/* missing switch selector */".to_string()));
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
            "switch_label" => labels.push(parse_switch_label(child, source)),
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

fn parse_switch_label(node: tree_sitter::Node, source: &str) -> SwitchLabel {
    first_named_child(node)
        .map(|label| SwitchLabel::Expr(parse_expr_node(label, source)))
        .unwrap_or(SwitchLabel::Default)
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
            let value = match op {
                Some("+=") => Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(target.clone()),
                    right: Box::new(value),
                },
                Some("-=") => Expr::Binary {
                    op: BinaryOp::Sub,
                    left: Box::new(target.clone()),
                    right: Box::new(value),
                },
                _ => value,
            };
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
                ty: ty.clone(),
                name: to_snake_case(&name),
                initializer,
            }
        })
        .collect()
}

fn parse_if_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    Stmt::If {
        condition: node
            .child_by_field_name("condition")
            .map(|condition| parse_condition_node(condition, source))
            .unwrap_or_else(|| Expr::Raw("true".to_string())),
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
            .unwrap_or_else(|| Expr::Raw("true".to_string())),
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
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
    Stmt::EnhancedFor {
        var_type: node
            .child_by_field_name("type")
            .map(|node| parse_type_node(node, source))
            .unwrap_or_else(|| JavaType::Unknown("enhanced_for".to_string())),
        var_name: node
            .child_by_field_name("name")
            .map(|node| to_snake_case(text(node, source)))
            .unwrap_or_else(|| "item".to_string()),
        iterable: node
            .child_by_field_name("value")
            .map(|node| parse_expr_node(node, source))
            .unwrap_or_else(|| Expr::Raw("/* missing iterable */".to_string())),
        body: node
            .child_by_field_name("body")
            .map(|stmt| statement_as_block(stmt, source, diagnostics))
            .unwrap_or_default(),
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
        first_named_child(node)
            .map(|child| parse_expr_node(child, source))
            .unwrap_or_else(|| Expr::Raw(text(node, source).to_string()))
    } else {
        parse_expr_node(node, source)
    }
}

fn parse_expr_node(node: tree_sitter::Node, source: &str) -> Expr {
    match node.kind() {
        "parenthesized_expression" => first_named_child(node)
            .map(|child| parse_expr_node(child, source))
            .unwrap_or_else(|| Expr::Raw(text(node, source).to_string())),
        "identifier" => Expr::Var(to_snake_case(text(node, source))),
        "this" => Expr::This,
        "super" => Expr::Super,
        "decimal_integer_literal" => parse_integer_literal(text(node, source)),
        "decimal_floating_point_literal" | "hex_floating_point_literal" => text(node, source)
            .trim_end_matches(['d', 'D'])
            .parse()
            .map(Literal::Double)
            .map(Expr::Literal)
            .unwrap_or_else(|_| Expr::Raw(text(node, source).to_string())),
        "true" => Expr::Literal(Literal::Bool(true)),
        "false" => Expr::Literal(Literal::Bool(false)),
        "null_literal" => Expr::Literal(Literal::Null),
        "string_literal" => {
            Expr::Literal(Literal::String(parse_string_literal(text(node, source))))
        }
        "character_literal" => Expr::Literal(Literal::Char(parse_char_literal(text(node, source)))),
        "field_access" => {
            let object = node
                .child_by_field_name("object")
                .map(|n| parse_expr_node(n, source));
            let field = field_text(node, "field", source).unwrap_or_else(|| "unknown".to_string());
            if let Some(object) = object {
                Expr::FieldAccess(Box::new(object), field)
            } else {
                Expr::Var(to_snake_case(&field))
            }
        }
        "array_access" => {
            let mut children = named_children(node).into_iter();
            let array = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| Expr::Raw("array".to_string()));
            let index = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| Expr::Raw("0".to_string()));
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
                _ => Expr::Raw(text(node, source).to_string()),
            }
        }
        "assignment_expression" => {
            let left = node
                .child_by_field_name("left")
                .map(|n| parse_expr_node(n, source));
            let right = node
                .child_by_field_name("right")
                .map(|n| parse_expr_node(n, source));
            match (left, right) {
                (Some(left), Some(right)) => Expr::Binary {
                    op: BinaryOp::Assign,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                _ => Expr::Raw(text(node, source).to_string()),
            }
        }
        "update_expression" => {
            let expr = first_named_child(node)
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| Expr::Raw(text(node, source).to_string()));
            let raw = text(node, source);
            let op = if raw.contains("--") {
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
                .unwrap_or_else(|| Expr::Raw(text(node, source).to_string()));
            let raw = text(node, source).trim_start();
            let op = if raw.starts_with('!') {
                UnaryOp::Not
            } else if raw.starts_with('-') {
                UnaryOp::Neg
            } else if raw.starts_with('+') {
                UnaryOp::Pos
            } else {
                return Expr::Raw(text(node, source).to_string());
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
                .unwrap_or_else(|| Expr::Raw("true".to_string()));
            let then_expr = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| Expr::Raw("()".to_string()));
            let else_expr = children
                .next()
                .map(|node| parse_expr_node(node, source))
                .unwrap_or_else(|| Expr::Raw("()".to_string()));
            Expr::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            }
        }
        "method_invocation" => {
            let receiver = node
                .child_by_field_name("object")
                .map(|object| Box::new(parse_expr_node(object, source)));
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
            let ty = node
                .child_by_field_name("type")
                .map(|node| parse_new_type_node(node, source))
                .unwrap_or_else(|| JavaType::Unknown("new".to_string()));
            Expr::New {
                ty,
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
                    .unwrap_or_else(|| Expr::Raw(text(node, source).to_string())),
            ),
        },
        "instanceof_expression" => Expr::InstanceOf {
            expr: Box::new(
                node.child_by_field_name("left")
                    .map(|left| parse_expr_node(left, source))
                    .unwrap_or_else(|| Expr::Raw("/* missing instanceof lhs */".to_string())),
            ),
            ty: node
                .child_by_field_name("right")
                .map(|right| parse_type_node(right, source))
                .unwrap_or_else(|| JavaType::Unknown("instanceof".to_string())),
        },
        "lambda_expression" => parse_lambda_node(node, source),
        "method_reference" => parse_method_reference_node(node, source),
        "class_literal" => Expr::ClassLiteral(
            first_named_child(node)
                .map(|ty| parse_type_node(ty, source))
                .unwrap_or_else(|| JavaType::Unknown("class".to_string())),
        ),
        _ => Expr::Raw(text(node, source).to_string()),
    }
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
    let field_ty = field_children(node, "type")
        .into_iter()
        .next()
        .map(|ty| parse_type_node(ty, source));
    let cast_text = text(node, source).trim();
    if let Some(close) = cast_text.find(')') {
        let ty_text = cast_text
            .get(1..close)
            .map(str::trim)
            .filter(|text| !text.is_empty());
        if let Some(ty_text) = ty_text {
            let parsed = parse_type_text(ty_text);
            if matches!(parsed, JavaType::Array(_)) || field_ty.is_none() {
                return parsed;
            }
        }
    }
    field_ty.unwrap_or_else(|| JavaType::Unknown("cast".to_string()))
}

fn parse_array_initializer_elements(node: tree_sitter::Node, source: &str) -> Vec<Expr> {
    named_children(node)
        .into_iter()
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
            _ => {}
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
        .unwrap_or_else(|| LambdaBody::Expr(Box::new(Expr::Raw(text(node, source).to_string()))));
    Expr::Lambda { params, body }
}

fn parse_lambda_params(node: tree_sitter::Node, source: &str) -> Vec<LambdaParam> {
    match node.kind() {
        "identifier" => vec![LambdaParam {
            name: to_snake_case(text(node, source)),
            ty: None,
        }],
        "inferred_parameters" => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == "identifier")
            .map(|id| LambdaParam {
                name: to_snake_case(text(id, source)),
                ty: None,
            })
            .collect(),
        "formal_parameters" => parse_params_node(node, source)
            .into_iter()
            .map(|param| LambdaParam {
                name: param.name,
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
    parse_type_text(text(node, source))
}

fn parse_new_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    let raw = text(node, source).trim();
    let mut ty = parse_type_text(raw);
    if type_text_starts_with_type_like_segment(raw) {
        if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &mut ty {
            *name = raw.to_string();
        }
    }
    ty
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
        .unwrap_or_else(|| Expr::Raw(src.to_string()))
}

fn collect_syntax_diagnostics(node: tree_sitter::Node, diagnostics: &mut DiagnosticBag) {
    match node.kind() {
        _ => {}
    }

    for child in named_children(node) {
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
        num.parse()
            .map(Literal::Long)
            .map(Expr::Literal)
            .unwrap_or_else(|_| Expr::Raw(src))
    } else {
        src.parse()
            .map(Literal::Int)
            .map(Expr::Literal)
            .unwrap_or_else(|_| Expr::Raw(src))
    }
}

fn parse_string_literal(src: &str) -> String {
    src.trim()
        .trim_start_matches('"')
        .trim_end_matches('"')
        .replace("\\\"", "\"")
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
        ">>" | ">>>" => BinaryOp::Shr,
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
    let Some(type_params) = declared_type_parameters_text(node, source) else {
        return Vec::new();
    };
    let Some(open) = type_params.find('<') else {
        return Vec::new();
    };
    let Some(close) = matching_angle_close(type_params, open) else {
        return Vec::new();
    };

    split_top_level_commas(&type_params[open + 1..close])
        .into_iter()
        .filter_map(|part| {
            part.split_whitespace()
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn parse_type_parameter_bounds(node: tree_sitter::Node, source: &str) -> Vec<Option<JavaType>> {
    let Some(type_params) = declared_type_parameters_text(node, source) else {
        return Vec::new();
    };
    let Some(open) = type_params.find('<') else {
        return Vec::new();
    };
    let Some(close) = matching_angle_close(type_params, open) else {
        return Vec::new();
    };

    split_top_level_commas(&type_params[open + 1..close])
        .into_iter()
        .map(|part| {
            part.split_once(" extends ")
                .map(|(_, bound)| parse_type_text(bound.split('&').next().unwrap_or(bound).trim()))
        })
        .collect()
}

fn declared_type_parameters_text<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    if node.kind() == "type_parameters" {
        return Some(text(node, source));
    }
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == "type_parameters")
        .map(|child| text(child, source))
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

pub fn to_snake_case(name: &str) -> String {
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
    match out.as_str() {
        "type" | "match" | "loop" | "crate" | "self" => format!("{out}_"),
        _ => out,
    }
}

pub fn type_suffix(ty: &JavaType, is_varargs: bool) -> String {
    let mut suffix = match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(p) => format!("{p:?}").to_ascii_lowercase(),
        JavaType::String => "string".to_string(),
        JavaType::Array(inner) => format!("{}_array", type_suffix(inner, false)),
        JavaType::Named { name, .. } => to_snake_case(name),
        JavaType::Wildcard(_) => "wildcard".to_string(),
        JavaType::Unknown(raw) => to_snake_case(raw),
    };
    if is_varargs {
        suffix.push_str("_varargs");
    }
    suffix
}
