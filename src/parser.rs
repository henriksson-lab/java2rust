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
    collect_syntax_diagnostics(root, source, &mut diagnostics);

    ParsedProgram {
        program: Program {
            package: parse_package(root, source),
            imports: parse_imports(root, source),
            types: parse_top_level_types(root, source, &mut diagnostics),
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
            text(node, source)
                .trim()
                .trim_start_matches("package")
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
            _ => {}
        }
    }
    if types.is_empty() {
        diagnostics.error("parse_no_class", "no top-level class found");
    }
    types
}

fn parse_class_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    let name = field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string());
    let extends = node.child_by_field_name("superclass").map(|superclass| {
        first_named_child(superclass)
            .map(|child| type_name_text(child, source))
            .unwrap_or_else(|| {
                text(superclass, source)
                    .trim_start_matches("extends")
                    .trim()
                    .to_string()
            })
    });
    if extends.is_some() {
        diagnostics.unsupported(
            "unsupported_inheritance",
            "inheritance is outside the first implementation slice",
        );
    }
    let implements = node
        .child_by_field_name("interfaces")
        .map(|interfaces| parse_type_name_list(interfaces, source))
        .unwrap_or_default();
    if !implements.is_empty() {
        diagnostics.unsupported(
            "unsupported_interface_impl",
            "interface implementation is outside the first implementation slice",
        );
    }

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
                _ => {}
            }
        }
    }

    TypeDecl {
        kind: TypeKind::Class,
        name,
        modifiers: parse_modifiers(node, source),
        extends,
        implements,
        members,
        span: span(node),
    }
}

fn parse_interface_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> TypeDecl {
    diagnostics.unsupported(
        "unsupported_interface",
        "interfaces are outside the first implementation slice",
    );
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
                _ => {}
            }
        }
    }

    TypeDecl {
        kind: TypeKind::Interface,
        name: field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string()),
        modifiers: parse_modifiers(node, source),
        extends: node
            .child_by_field_name("interfaces")
            .and_then(|interfaces| parse_type_name_list(interfaces, source).into_iter().next()),
        implements: Vec::new(),
        members,
        span: span(node),
    }
}

fn parse_field_decl(node: tree_sitter::Node, source: &str) -> Vec<Member> {
    let ty = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("field".to_string()));
    let modifiers = parse_modifiers(node, source);
    field_children(node, "declarator")
        .into_iter()
        .filter_map(|decl| {
            let name = field_text(decl, "name", source)?;
            let initializer = decl
                .child_by_field_name("value")
                .map(|value| parse_expr_node(value, source));
            Some(Member::Field(FieldDecl {
                name: to_snake_case(&name),
                ty: ty.clone(),
                modifiers: modifiers.clone(),
                initializer,
                span: span(decl),
            }))
        })
        .collect()
}

fn parse_constructor_decl(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) -> ConstructorDecl {
    ConstructorDecl {
        name: field_text(node, "name", source).unwrap_or_else(|| "Unknown".to_string()),
        rust_name: None,
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
    named_children(node)
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
                .unwrap_or_else(|| JavaType::Unknown(name.clone()));
            Some(Param {
                name: to_snake_case(&name),
                ty,
                is_varargs,
            })
        })
        .collect()
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
            "statement" => {
                if let Some(stmt) = first_named_child(child) {
                    statements.push(parse_stmt_node(stmt, source, diagnostics));
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
            | "throw_statement"
            | "switch_expression"
            | "switch_statement"
            | "break_statement"
            | "continue_statement"
            | "block"
            | "class_declaration"
            | "interface_declaration"
            | "explicit_constructor_invocation" => {
                statements.push(parse_stmt_node(child, source, diagnostics));
            }
            _ => {}
        }
    }
    Block { statements }
}

fn parse_stmt_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    match node.kind() {
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
        "try_statement" => parse_try_node(node, source, diagnostics),
        "throw_statement" => first_named_child(node)
            .map(|expr| Stmt::Throw(parse_expr_node(expr, source)))
            .unwrap_or_else(|| Stmt::Unsupported("throw without expression".to_string())),
        "switch_expression" | "switch_block" | "switch_statement" => {
            parse_switch_node(node, source, diagnostics)
        }
        "class_declaration" => Stmt::LocalType(parse_class_decl(node, source, diagnostics)),
        "interface_declaration" => Stmt::LocalType(parse_interface_decl(node, source, diagnostics)),
        "explicit_constructor_invocation" => {
            parse_constructor_invocation(node, source, diagnostics)
        }
        _ => Stmt::Unsupported(format!("unsupported statement node: {}", node.kind())),
    }
}

fn parse_try_node(node: tree_sitter::Node, source: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    diagnostics.unsupported(
        "unsupported_exceptions",
        "try/catch is outside the first implementation slice",
    );
    let body = node
        .child_by_field_name("body")
        .map(|body| parse_block_like(body, source, diagnostics))
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
        body,
        catches,
        finally_block,
    }
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
            "statement" => {
                if let Some(stmt) = first_named_child(child) {
                    statements.push(parse_stmt_node(stmt, source, diagnostics));
                }
            }
            _ => statements.push(parse_stmt_node(child, source, diagnostics)),
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
    diagnostics: &mut DiagnosticBag,
) -> Stmt {
    let kind = match node.child_by_field_name("constructor") {
        Some(constructor) if constructor.kind() == "super" => ConstructorInvocationKind::Super,
        _ => ConstructorInvocationKind::This,
    };
    if matches!(kind, ConstructorInvocationKind::Super) {
        diagnostics.unsupported(
            "unsupported_constructor_super",
            "super constructor delegation requires inheritance lowering",
        );
    }
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
    let ty = node
        .child_by_field_name("type")
        .map(|node| parse_type_node(node, source))
        .unwrap_or_else(|| JavaType::Unknown("local".to_string()));
    let Some(declarator) = field_children(node, "declarator").into_iter().next() else {
        return Stmt::Unsupported("local variable without declarator".to_string());
    };
    let name = field_text(declarator, "name", source).unwrap_or_else(|| "unknown".to_string());
    let initializer = declarator
        .child_by_field_name("value")
        .map(|value| parse_expr_node(value, source));
    Stmt::VarDecl {
        ty,
        name: to_snake_case(&name),
        initializer,
    }
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
        .map(|init| {
            if init.kind() == "local_variable_declaration" {
                parse_local_var_decl(init, source)
            } else {
                Stmt::Expr(parse_expr_node(init, source))
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
            .map(|stmt| Block {
                statements: vec![parse_stmt_node(stmt, source, diagnostics)],
            })
            .unwrap_or_default()
    } else {
        Block {
            statements: vec![parse_stmt_node(node, source, diagnostics)],
        }
    }
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
        "character_literal" => Expr::Raw(text(node, source).to_string()),
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
                    op: BinaryOp::Add,
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
                .map(|node| parse_type_node(node, source))
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
            }
        }
        "cast_expression" => Expr::Cast {
            ty: field_children(node, "type")
                .into_iter()
                .next()
                .map(|ty| parse_type_node(ty, source))
                .unwrap_or_else(|| JavaType::Unknown("cast".to_string())),
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
        .map(|arg| parse_expr_node(arg, source))
        .collect()
}

pub fn parse_type(src: &str) -> JavaType {
    parse_type_text(src)
}

fn parse_type_node(node: tree_sitter::Node, source: &str) -> JavaType {
    parse_type_text(text(node, source))
}

fn parse_type_text(src: &str) -> JavaType {
    let src = src.trim();
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
                let name = src[..open]
                    .trim()
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_string();
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
                name: src.rsplit('.').next().unwrap_or(src).to_string(),
                args: Vec::new(),
            }
        }
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

fn collect_syntax_diagnostics(
    node: tree_sitter::Node,
    source: &str,
    diagnostics: &mut DiagnosticBag,
) {
    match node.kind() {
        "interface_declaration" => diagnostics.unsupported(
            "unsupported_interface",
            "interfaces are outside the first implementation slice",
        ),
        "superclass" => diagnostics.unsupported(
            "unsupported_inheritance",
            "inheritance is outside the first implementation slice",
        ),
        "super_interfaces" => diagnostics.unsupported(
            "unsupported_interface_impl",
            "interface implementation is outside the first implementation slice",
        ),
        "catch_clause" => diagnostics.unsupported(
            "unsupported_exceptions",
            "catch clauses are outside the first exception lowering slice",
        ),
        "finally_clause" => diagnostics.unsupported(
            "unsupported_finally",
            "finally blocks are outside the first implementation slice",
        ),
        "wildcard" | "wildcard_bounds" => diagnostics.unsupported(
            "unsupported_wildcard",
            "wildcards are outside the first implementation slice",
        ),
        _ => {}
    }

    for child in named_children(node) {
        collect_syntax_diagnostics(child, source, diagnostics);
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

fn parse_binary_op(src: &str) -> Option<BinaryOp> {
    Some(match src {
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Sub,
        "*" => BinaryOp::Mul,
        "/" => BinaryOp::Div,
        "%" => BinaryOp::Rem,
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
