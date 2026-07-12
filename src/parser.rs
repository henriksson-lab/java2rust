use crate::diagnostics::{DiagnosticBag, Span};
use crate::java_ir::*;

#[derive(Debug, Clone)]
pub struct ParsedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
}

pub fn parse_java_source(source: &str) -> ParsedProgram {
    let mut diagnostics = DiagnosticBag::new();
    detect_guardrail_constructs(source, &mut diagnostics);

    let package = parse_package(source);
    let imports = parse_imports(source);
    let types = parse_types(source, &mut diagnostics);

    ParsedProgram {
        program: Program {
            package,
            imports,
            types,
        },
        diagnostics,
    }
}

fn parse_package(source: &str) -> Option<String> {
    source
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("package "))
        .and_then(|rest| rest.strip_suffix(';').or(Some(rest)))
        .map(|s| s.trim().to_string())
}

fn parse_imports(source: &str) -> Vec<ImportDecl> {
    source
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("import "))
        .filter_map(|rest| {
            let rest = rest.strip_suffix(';')?.trim();
            let (is_static, path) = if let Some(path) = rest.strip_prefix("static ") {
                (true, path.trim())
            } else {
                (false, rest)
            };
            Some(ImportDecl {
                path: path.trim_end_matches(".*").to_string(),
                is_static,
                is_wildcard: path.ends_with(".*"),
            })
        })
        .collect()
}

fn detect_guardrail_constructs(source: &str, diagnostics: &mut DiagnosticBag) {
    let guards = [
        (
            " extends ",
            "unsupported_inheritance",
            "inheritance is outside the first implementation slice",
        ),
        (
            " implements ",
            "unsupported_interface_impl",
            "interface implementation is outside the first implementation slice",
        ),
        (
            " interface ",
            "unsupported_interface",
            "interfaces are outside the first implementation slice",
        ),
        (
            " throws ",
            "unsupported_exceptions",
            "checked exceptions are outside the first implementation slice",
        ),
        (
            " catch ",
            "unsupported_exceptions",
            "catch blocks are outside the first implementation slice",
        ),
        (
            " finally ",
            "unsupported_finally",
            "finally blocks are outside the first implementation slice",
        ),
        (
            " -> ",
            "unsupported_lambda",
            "lambdas are outside the first implementation slice",
        ),
        (
            "<?>",
            "unsupported_wildcard",
            "wildcards are outside the first implementation slice",
        ),
        (
            "? extends",
            "unsupported_wildcard",
            "wildcards are outside the first implementation slice",
        ),
        (
            "? super",
            "unsupported_wildcard",
            "wildcards are outside the first implementation slice",
        ),
        (
            ".stream()",
            "unsupported_stream",
            "streams are outside the first implementation slice",
        ),
        (
            " new ",
            "unsupported_new_expression",
            "object creation is only partially supported in the first implementation slice",
        ),
    ];
    for (needle, code, message) in guards {
        if source.contains(needle) {
            diagnostics.unsupported(code, message);
        }
    }
}

fn parse_types(source: &str, diagnostics: &mut DiagnosticBag) -> Vec<TypeDecl> {
    let Some(class_idx) = source.find(" class ") else {
        diagnostics.error("parse_no_class", "no top-level class found");
        return Vec::new();
    };

    let header_start = source[..class_idx]
        .rfind(|c: char| c == '\n' || c == ';' || c == '}')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let Some(open_brace) = source[class_idx..].find('{').map(|i| class_idx + i) else {
        diagnostics.error("parse_class_body", "class has no body");
        return Vec::new();
    };
    let Some(close_brace) = find_matching_brace(source, open_brace) else {
        diagnostics.error("parse_class_body", "class body is not balanced");
        return Vec::new();
    };

    let header = source[header_start..open_brace].trim();
    let name = header
        .split_whitespace()
        .skip_while(|tok| *tok != "class")
        .nth(1)
        .unwrap_or("Unknown")
        .to_string();
    let modifiers = parse_modifiers(header);
    let extends = header
        .split(" extends ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .map(|s| s.trim_end_matches('{').to_string());
    let body = &source[open_brace + 1..close_brace];
    let members = parse_members(body, &name, diagnostics);

    vec![TypeDecl {
        kind: TypeKind::Class,
        name,
        modifiers,
        extends,
        implements: Vec::new(),
        members,
        span: Span::new(header_start, close_brace + 1),
    }]
}

fn parse_members(body: &str, class_name: &str, diagnostics: &mut DiagnosticBag) -> Vec<Member> {
    let mut members = Vec::new();
    for chunk in top_level_chunks(body) {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains(" class ")
            || trimmed.starts_with("class ")
            || trimmed.contains(" interface ")
        {
            members.push(Member::Unsupported {
                reason: "nested/local/anonymous types are represented but not lowered in the first slice".to_string(),
                span: Span::default(),
            });
            continue;
        }
        if let Some(member) = parse_callable_or_field(trimmed, class_name, diagnostics) {
            members.push(member);
        }
    }
    members
}

fn parse_callable_or_field(
    chunk: &str,
    class_name: &str,
    diagnostics: &mut DiagnosticBag,
) -> Option<Member> {
    if let Some(open) = chunk.find('{') {
        let signature = chunk[..open].trim();
        let body_src = chunk[open + 1..chunk.rfind('}').unwrap_or(chunk.len())].trim();
        return parse_callable(signature, body_src, class_name, diagnostics);
    }

    let line = chunk.trim_end_matches(';').trim();
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 2 {
        return None;
    }
    let modifiers = parse_modifiers(line);
    let non_mod = tokens
        .iter()
        .copied()
        .filter(|tok| !is_modifier(tok))
        .collect::<Vec<_>>();
    if non_mod.len() < 2 {
        return None;
    }
    let ty = parse_type(non_mod[0]);
    let name_and_init = non_mod[1..].join(" ");
    let (name, initializer) = if let Some((name, init)) = name_and_init.split_once('=') {
        (name.trim().to_string(), Some(parse_expr(init.trim())))
    } else {
        (name_and_init.trim().to_string(), None)
    };
    Some(Member::Field(FieldDecl {
        name: to_snake_case(&name),
        ty,
        modifiers,
        initializer,
        span: Span::default(),
    }))
}

fn parse_callable(
    signature: &str,
    body_src: &str,
    class_name: &str,
    diagnostics: &mut DiagnosticBag,
) -> Option<Member> {
    let Some(open_paren) = signature.find('(') else {
        return None;
    };
    let close_paren = signature.rfind(')').unwrap_or(signature.len());
    let before = signature[..open_paren].trim();
    let params = parse_params(&signature[open_paren + 1..close_paren]);
    let tokens = before.split_whitespace().collect::<Vec<_>>();
    let modifiers = parse_modifiers(before);
    let non_mod = tokens
        .iter()
        .copied()
        .filter(|tok| !is_modifier(tok))
        .collect::<Vec<_>>();
    if non_mod.is_empty() {
        return None;
    }
    let block = Some(parse_block(body_src, diagnostics));
    if non_mod.len() == 1 && non_mod[0] == class_name {
        return Some(Member::Constructor(ConstructorDecl {
            name: class_name.to_string(),
            rust_name: None,
            params,
            modifiers,
            body: block,
            span: Span::default(),
        }));
    }
    if non_mod.len() < 2 {
        return None;
    }
    let return_type = parse_type(non_mod[0]);
    let name = non_mod[1].to_string();
    Some(Member::Method(MethodDecl {
        name,
        rust_name: None,
        return_type,
        params,
        modifiers,
        body: block,
        span: Span::default(),
    }))
}

fn parse_params(src: &str) -> Vec<Param> {
    src.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|param| {
            let parts = param.split_whitespace().collect::<Vec<_>>();
            if parts.len() < 2 {
                return None;
            }
            let name = parts.last()?.to_string();
            let mut ty_src = parts[..parts.len() - 1].join(" ");
            let is_varargs = ty_src.contains("...");
            ty_src = ty_src.replace("...", "[]");
            Some(Param {
                name: to_snake_case(&name),
                ty: parse_type(&ty_src),
                is_varargs,
            })
        })
        .collect()
}

fn parse_block(src: &str, diagnostics: &mut DiagnosticBag) -> Block {
    let mut statements = Vec::new();
    for chunk in statement_chunks(src) {
        let stmt = parse_stmt(chunk.trim(), diagnostics);
        statements.push(stmt);
    }
    Block { statements }
}

fn parse_stmt(src: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let src = src.trim();
    if src.is_empty() {
        return Stmt::Unsupported("empty statement".to_string());
    }
    if let Some(expr) = src.strip_prefix("return ") {
        return Stmt::Return(Some(parse_expr(expr.trim_end_matches(';').trim())));
    }
    if src == "return;" {
        return Stmt::Return(None);
    }
    if src == "break;" {
        return Stmt::Break(None);
    }
    if src == "continue;" {
        return Stmt::Continue(None);
    }
    if let Some(rest) = src.strip_prefix("if ") {
        return parse_if(rest, diagnostics);
    }
    if let Some(rest) = src.strip_prefix("for ") {
        return parse_for(rest, diagnostics);
    }
    if let Some(rest) = src.strip_prefix("while ") {
        return parse_while(rest, diagnostics);
    }
    if is_var_decl(src) {
        return parse_var_decl(src);
    }
    if let Some((target, value)) = split_assignment(src.trim_end_matches(';')) {
        return Stmt::Assign {
            target: parse_expr(target.trim()),
            value: parse_expr(value.trim()),
        };
    }
    Stmt::Expr(parse_expr(src.trim_end_matches(';').trim()))
}

fn parse_if(src: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let Some(open) = src.find('(') else {
        return Stmt::Unsupported(format!("unsupported if: {src}"));
    };
    let close = find_matching_char(src, open, '(', ')').unwrap_or(open);
    let condition = parse_expr(&src[open + 1..close]);
    let rest = src[close + 1..].trim();
    let then_block = parse_braced_block(rest, diagnostics).unwrap_or_default();
    Stmt::If {
        condition,
        then_block,
        else_block: None,
    }
}

fn parse_for(src: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let Some(open) = src.find('(') else {
        return Stmt::Unsupported(format!("unsupported for: {src}"));
    };
    let close = find_matching_char(src, open, '(', ')').unwrap_or(open);
    let header = &src[open + 1..close];
    let body = parse_braced_block(src[close + 1..].trim(), diagnostics).unwrap_or_default();
    if header.contains(':') {
        let (left, iterable) = header.split_once(':').unwrap();
        let parts = left.split_whitespace().collect::<Vec<_>>();
        if parts.len() >= 2 {
            return Stmt::EnhancedFor {
                var_type: parse_type(parts[0]),
                var_name: to_snake_case(parts[1]),
                iterable: parse_expr(iterable.trim()),
                body,
            };
        }
    }
    let parts = header.split(';').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Stmt::Unsupported(format!("unsupported for header: {header}"));
    }
    let init = if parts[0].trim().is_empty() {
        Vec::new()
    } else {
        vec![parse_stmt(&format!("{};", parts[0].trim()), diagnostics)]
    };
    let condition = if parts[1].trim().is_empty() {
        None
    } else {
        Some(parse_expr(parts[1].trim()))
    };
    let update = if parts[2].trim().is_empty() {
        Vec::new()
    } else {
        vec![parse_expr(parts[2].trim())]
    };
    Stmt::For {
        init,
        condition,
        update,
        body,
        label: None,
    }
}

fn parse_while(src: &str, diagnostics: &mut DiagnosticBag) -> Stmt {
    let Some(open) = src.find('(') else {
        return Stmt::Unsupported(format!("unsupported while: {src}"));
    };
    let close = find_matching_char(src, open, '(', ')').unwrap_or(open);
    Stmt::While {
        condition: parse_expr(&src[open + 1..close]),
        body: parse_braced_block(src[close + 1..].trim(), diagnostics).unwrap_or_default(),
    }
}

fn parse_braced_block(src: &str, diagnostics: &mut DiagnosticBag) -> Option<Block> {
    let open = src.find('{')?;
    let close = find_matching_char(src, open, '{', '}')?;
    Some(parse_block(&src[open + 1..close], diagnostics))
}

fn parse_var_decl(src: &str) -> Stmt {
    let src = src.trim_end_matches(';').trim();
    let (left, initializer) = if let Some((left, init)) = src.split_once('=') {
        (left.trim(), Some(parse_expr(init.trim())))
    } else {
        (src, None)
    };
    let parts = left.split_whitespace().collect::<Vec<_>>();
    Stmt::VarDecl {
        ty: parse_type(parts[0]),
        name: to_snake_case(parts[1]),
        initializer,
    }
}

fn is_var_decl(src: &str) -> bool {
    let first = src.split_whitespace().next().unwrap_or("");
    matches!(
        first,
        "boolean" | "byte" | "short" | "char" | "int" | "long" | "float" | "double" | "String"
    ) || first.ends_with("[]")
}

fn split_assignment(src: &str) -> Option<(&str, &str)> {
    for op in ["+=", "-=", "*=", "/=", "="] {
        if let Some(idx) = find_top_level(src, op) {
            if op == "=" && src[..idx].ends_with(['!', '<', '>', '=']) {
                continue;
            }
            let rhs = &src[idx + op.len()..];
            if op == "+=" {
                return Some((
                    &src[..idx],
                    Box::leak(format!("{} + {}", &src[..idx], rhs).into_boxed_str()),
                ));
            }
            return Some((&src[..idx], rhs));
        }
    }
    None
}

pub fn parse_expr(src: &str) -> Expr {
    let src = src.trim();
    if src.is_empty() {
        return Expr::Raw(String::new());
    }
    if src.ends_with("++") {
        return Expr::Unary {
            op: UnaryOp::PostInc,
            expr: Box::new(parse_expr(src.trim_end_matches("++"))),
        };
    }
    if src.ends_with("--") {
        return Expr::Unary {
            op: UnaryOp::PostDec,
            expr: Box::new(parse_expr(src.trim_end_matches("--"))),
        };
    }
    if let Some((c, t, e)) = split_ternary(src) {
        return Expr::Ternary {
            condition: Box::new(parse_expr(c)),
            then_expr: Box::new(parse_expr(t)),
            else_expr: Box::new(parse_expr(e)),
        };
    }
    for (needle, op) in [
        ("||", BinaryOp::Or),
        ("&&", BinaryOp::And),
        ("==", BinaryOp::Eq),
        ("!=", BinaryOp::Ne),
        (">=", BinaryOp::Ge),
        ("<=", BinaryOp::Le),
        ("<<", BinaryOp::Shl),
        (">>", BinaryOp::Shr),
        (">", BinaryOp::Gt),
        ("<", BinaryOp::Lt),
        ("+", BinaryOp::Add),
        ("-", BinaryOp::Sub),
        ("*", BinaryOp::Mul),
        ("/", BinaryOp::Div),
        ("%", BinaryOp::Rem),
    ] {
        if let Some(idx) = find_top_level_right(src, needle) {
            return Expr::Binary {
                op,
                left: Box::new(parse_expr(&src[..idx])),
                right: Box::new(parse_expr(&src[idx + needle.len()..])),
            };
        }
    }
    if src.starts_with('(') && find_matching_char(src, 0, '(', ')') == Some(src.len() - 1) {
        return parse_expr(&src[1..src.len() - 1]);
    }
    if let Some(lit) = parse_literal(src) {
        return Expr::Literal(lit);
    }
    if let Some(idx) = find_top_level(src, "[") {
        if src.ends_with(']') {
            return Expr::ArrayAccess(
                Box::new(parse_expr(&src[..idx])),
                Box::new(parse_expr(&src[idx + 1..src.len() - 1])),
            );
        }
    }
    if let Some((recv, field)) = split_field_access(src) {
        if field.ends_with(')') && field.contains('(') {
            let open = field.find('(').unwrap();
            let name = &field[..open];
            let args = parse_args(&field[open + 1..field.len() - 1]);
            return Expr::Call {
                receiver: Some(Box::new(parse_expr(recv))),
                name: name.to_string(),
                args,
            };
        }
        return Expr::FieldAccess(Box::new(parse_expr(recv)), field.to_string());
    }
    if src.ends_with(')') && src.contains('(') {
        let open = src.find('(').unwrap();
        let name = &src[..open];
        return Expr::Call {
            receiver: None,
            name: name.to_string(),
            args: parse_args(&src[open + 1..src.len() - 1]),
        };
    }
    if src == "this" {
        return Expr::This;
    }
    Expr::Var(to_snake_case(src))
}

fn parse_args(src: &str) -> Vec<Expr> {
    split_top_level_commas(src)
        .into_iter()
        .map(|arg| parse_expr(arg.trim()))
        .collect()
}

fn parse_literal(src: &str) -> Option<Literal> {
    if src == "null" {
        Some(Literal::Null)
    } else if src == "true" {
        Some(Literal::Bool(true))
    } else if src == "false" {
        Some(Literal::Bool(false))
    } else if src.starts_with('"') && src.ends_with('"') {
        Some(Literal::String(src[1..src.len() - 1].to_string()))
    } else if let Some(num) = src.strip_suffix('L') {
        num.parse().ok().map(Literal::Long)
    } else if src.contains('.') {
        src.trim_end_matches(['d', 'D'])
            .parse()
            .ok()
            .map(Literal::Double)
    } else {
        src.parse().ok().map(Literal::Int)
    }
}

pub fn parse_type(src: &str) -> JavaType {
    let src = src.trim();
    if let Some(inner) = src.strip_suffix("[]") {
        return JavaType::Array(Box::new(parse_type(inner)));
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
        _ => {
            if let Some(open) = src.find('<') {
                let name = src[..open]
                    .trim()
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let inner = src[open + 1..src.rfind('>').unwrap_or(src.len())].trim();
                JavaType::Named {
                    name,
                    args: split_top_level_commas(inner)
                        .into_iter()
                        .map(|part| parse_type(part.trim()))
                        .collect(),
                }
            } else {
                JavaType::Named {
                    name: src.rsplit('.').next().unwrap_or(src).to_string(),
                    args: Vec::new(),
                }
            }
        }
    }
}

fn parse_modifiers(src: &str) -> Vec<Modifier> {
    src.split_whitespace()
        .filter_map(|tok| match tok {
            "public" => Some(Modifier::Public),
            "protected" => Some(Modifier::Protected),
            "private" => Some(Modifier::Private),
            "static" => Some(Modifier::Static),
            "final" => Some(Modifier::Final),
            "abstract" => Some(Modifier::Abstract),
            _ => None,
        })
        .collect()
}

fn is_modifier(tok: &str) -> bool {
    matches!(
        tok,
        "public" | "protected" | "private" | "static" | "final" | "abstract"
    )
}

fn top_level_chunks(body: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0i32;
    let mut paren_depth = 0i32;
    for (idx, ch) in body.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 && paren_depth == 0 {
                    chunks.push(&body[start..=idx]);
                    start = idx + 1;
                }
            }
            ';' if brace_depth == 0 && paren_depth == 0 => {
                chunks.push(&body[start..=idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    chunks
}

fn statement_chunks(body: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0i32;
    let mut paren_depth = 0i32;
    for (idx, ch) in body.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 && paren_depth == 0 {
                    chunks.push(&body[start..=idx]);
                    start = idx + 1;
                }
            }
            ';' if brace_depth == 0 && paren_depth == 0 => {
                chunks.push(&body[start..=idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    chunks
}

fn split_ternary(src: &str) -> Option<(&str, &str, &str)> {
    let q = find_top_level(src, "?")?;
    let c = find_top_level(&src[q + 1..], ":")? + q + 1;
    Some((&src[..q], &src[q + 1..c], &src[c + 1..]))
}

fn split_field_access(src: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (idx, ch) in src.char_indices().rev() {
        match ch {
            ')' | ']' => depth += 1,
            '(' | '[' => depth -= 1,
            '.' if depth == 0 => return Some((&src[..idx], &src[idx + 1..])),
            _ => {}
        }
    }
    None
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

fn find_top_level(src: &str, needle: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in src.char_indices() {
        if depth == 0 && src[idx..].starts_with(needle) {
            return Some(idx);
        }
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn find_top_level_right(src: &str, needle: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in src.char_indices().rev() {
        match ch {
            ')' | ']' => depth += 1,
            '(' | '[' => depth -= 1,
            _ => {}
        }
        if depth == 0 && src[idx..].starts_with(needle) {
            return Some(idx);
        }
    }
    None
}

fn find_matching_brace(src: &str, open: usize) -> Option<usize> {
    find_matching_char(src, open, '{', '}')
}

fn find_matching_char(src: &str, open: usize, left: char, right: char) -> Option<usize> {
    let mut depth = 0i32;
    for (idx, ch) in src[open..].char_indices() {
        match ch {
            c if c == left => depth += 1,
            c if c == right => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + idx);
                }
            }
            _ => {}
        }
    }
    None
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
