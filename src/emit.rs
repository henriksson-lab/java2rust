use crate::rust_ir::*;

pub fn emit_file(file: &RustFile) -> String {
    let mut out = String::new();
    for item in &file.items {
        out.push_str(&emit_item(item));
        out.push('\n');
    }
    out
}

fn emit_item(item: &RustItem) -> String {
    match item {
        RustItem::Struct(strukt) => emit_struct(strukt),
        RustItem::Impl(imp) => emit_impl(imp),
        RustItem::Function(function) => emit_function(function, 0),
        RustItem::Comment(comment) => format!("// {comment}\n"),
        RustItem::Raw(raw) => format!("{raw}\n"),
    }
}

fn emit_struct(strukt: &RustStruct) -> String {
    let mut out = String::new();
    if !strukt.derives.is_empty() {
        out.push_str(&format!("#[derive({})]\n", strukt.derives.join(", ")));
    }
    out.push_str(&format!("pub struct {} {{\n", rust_ident(&strukt.name)));
    for field in &strukt.fields {
        let vis = if field.is_pub {
            "pub "
        } else if field.is_pub_crate {
            "pub(crate) "
        } else {
            ""
        };
        out.push_str(&format!(
            "    {vis}{}: {},\n",
            rust_ident(&field.name),
            emit_type(&field.ty)
        ));
    }
    out.push_str("}\n");
    out
}

fn emit_impl(imp: &RustImpl) -> String {
    let mut out = format!("impl {} {{\n", rust_ident(&imp.target));
    for associated_const in &imp.associated_consts {
        let vis = if associated_const.is_pub { "pub " } else { "" };
        out.push_str(&format!(
            "    {vis}const {}: {} = {};\n\n",
            rust_ident(&associated_const.name),
            emit_type(&associated_const.ty),
            emit_expr(&associated_const.value)
        ));
    }
    for function in &imp.functions {
        out.push_str(&emit_function(function, 1));
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

fn emit_function(function: &RustFunction, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    let mut params = Vec::new();
    if let Some(receiver) = function.receiver {
        params.push(match receiver {
            RustReceiver::Shared => "&self".to_string(),
            RustReceiver::Mutable => "&mut self".to_string(),
        });
    }
    params.extend(
        function
            .params
            .iter()
            .map(|p| format!("{}: {}", rust_param_ident(&p.name), emit_type(&p.ty))),
    );
    let vis = if function.is_pub { "pub " } else { "" };
    let ret = function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", emit_type(ty)))
        .unwrap_or_default();
    let mut out = format!(
        "{pad}{vis}fn {}({}){ret} {{\n",
        rust_ident(&function.name),
        params.join(", ")
    );
    out.push_str(&emit_statements(&function.body, indent + 1));
    out.push_str(&format!("{pad}}}\n"));
    out
}

pub fn emit_statements(statements: &[RustStmt], indent: usize) -> String {
    statements
        .iter()
        .map(|stmt| emit_stmt(stmt, indent))
        .collect::<String>()
}

fn emit_stmt(stmt: &RustStmt, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match stmt {
        RustStmt::Let {
            mutable,
            name,
            ty,
            value,
        } => {
            let mutability = if *mutable { "mut " } else { "" };
            let ty = ty
                .as_ref()
                .map(|ty| format!(": {}", emit_type(ty)))
                .unwrap_or_default();
            let value = value
                .as_ref()
                .map(|expr| format!(" = {}", emit_expr(expr)))
                .unwrap_or_default();
            format!("{pad}let {mutability}{}{ty}{value};\n", rust_ident(name))
        }
        RustStmt::Assign { target, value } => {
            format!("{pad}{} = {};\n", emit_expr(target), emit_expr(value))
        }
        RustStmt::Expr(expr) if is_unit_expr(expr) => String::new(),
        RustStmt::Expr(expr) => format!("{pad}{};\n", emit_expr(expr)),
        RustStmt::Return(Some(expr)) if is_unit_expr(expr) => format!("{pad}return;\n"),
        RustStmt::Return(Some(expr)) => format!("{pad}return {};\n", emit_expr(expr)),
        RustStmt::Return(None) => format!("{pad}return;\n"),
        RustStmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let mut out = format!("{pad}if {} {{\n", emit_expr(condition));
            out.push_str(&emit_statements(then_body, indent + 1));
            out.push_str(&format!("{pad}}}"));
            if !else_body.is_empty() {
                out.push_str(" else {\n");
                out.push_str(&emit_statements(else_body, indent + 1));
                out.push_str(&format!("{pad}}}"));
            }
            out.push('\n');
            out
        }
        RustStmt::While { condition, body } => {
            let mut out = format!("{pad}while {} {{\n", emit_expr(condition));
            out.push_str(&emit_statements(body, indent + 1));
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        RustStmt::Loop { label, body } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{l}: "))
                .unwrap_or_default();
            let mut out = format!("{pad}{label}loop {{\n");
            out.push_str(&emit_statements(body, indent + 1));
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        RustStmt::Break(label) => {
            let label = label.as_ref().map(|l| format!(" '{l}")).unwrap_or_default();
            format!("{pad}break{label};\n")
        }
        RustStmt::Continue(label) => {
            let label = label.as_ref().map(|l| format!(" '{l}")).unwrap_or_default();
            format!("{pad}continue{label};\n")
        }
        RustStmt::Raw(raw) => raw
            .lines()
            .map(|line| format!("{pad}{line}\n"))
            .collect::<String>(),
    }
}

pub fn emit_expr(expr: &RustExpr) -> String {
    emit_expr_prec(expr, 0, None, ExprChildSide::None)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExprChildSide {
    None,
    Left,
    Right,
}

fn emit_expr_prec(
    expr: &RustExpr,
    parent_prec: u8,
    parent_op: Option<&str>,
    child_side: ExprChildSide,
) -> String {
    let rendered = match expr {
        RustExpr::Literal(value) => value.clone(),
        RustExpr::Var(name) => rust_ident(name),
        RustExpr::Field(receiver, field) => {
            format!(
                "{}.{}",
                emit_expr_prec(receiver, 11, None, ExprChildSide::None),
                rust_ident(field)
            )
        }
        RustExpr::Index(array, index) => {
            format!(
                "{}[{}]",
                emit_expr_prec(array, 11, None, ExprChildSide::None),
                emit_expr_prec(index, 0, None, ExprChildSide::None)
            )
        }
        RustExpr::Binary { op, left, right } => format!(
            "{} {op} {}",
            emit_expr_prec(left, binary_precedence(op), Some(op), ExprChildSide::Left),
            emit_expr_prec(right, binary_precedence(op), Some(op), ExprChildSide::Right)
        ),
        RustExpr::Unary { op, expr } => {
            format!(
                "{op}{}",
                emit_expr_prec(expr, 10, None, ExprChildSide::None)
            )
        }
        RustExpr::Cast { expr, ty } => {
            format!(
                "{} as {ty}",
                emit_expr_prec(expr, 11, None, ExprChildSide::None)
            )
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => format!(
            "if {} {{ {} }} else {{ {} }}",
            emit_expr_prec(condition, 0, None, ExprChildSide::None),
            emit_expr_prec(then_expr, 0, None, ExprChildSide::None),
            emit_expr_prec(else_expr, 0, None, ExprChildSide::None)
        ),
        RustExpr::Call { function, args } => format!(
            "{}({})",
            emit_expr_prec(function, 11, None, ExprChildSide::None),
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => format!(
            "{}.{}({})",
            emit_expr_prec(receiver, 11, None, ExprChildSide::None),
            rust_ident(method),
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::StructLit { name, fields } => {
            let fields = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", rust_ident(name), emit_expr(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {fields} }}", rust_ident(name))
        }
        RustExpr::Raw(raw) => normalize_raw_expr_for_context(raw, parent_prec),
    };

    if expr_needs_parentheses(expr, parent_prec, parent_op, child_side) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn rust_ident(name: &str) -> String {
    if name.starts_with("r#")
        || name.contains("::")
        || name.contains('<')
        || name.contains('>')
        || name.contains(' ')
        || name.is_empty()
    {
        return name.to_string();
    }
    if is_raw_identifier_keyword(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn rust_param_ident(name: &str) -> String {
    if let Some(inner) = name.strip_prefix("mut ") {
        return format!("mut {}", rust_ident(inner));
    }
    rust_ident(name)
}

fn is_raw_identifier_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

fn expr_needs_parentheses(
    expr: &RustExpr,
    parent_prec: u8,
    parent_op: Option<&str>,
    child_side: ExprChildSide,
) -> bool {
    let own_prec = expr_precedence(expr);
    own_prec < parent_prec
        || match (expr, parent_op, child_side) {
            (RustExpr::Binary { op: child_op, .. }, Some(parent_op), ExprChildSide::Left) => {
                (arithmetic_binary_op(parent_op) && arithmetic_binary_op(child_op))
                    || (bitwise_binary_op(parent_op) && comparison_binary_op(child_op))
            }
            (RustExpr::Binary { op: child_op, .. }, Some(parent_op), ExprChildSide::Right)
                if own_prec == parent_prec =>
            {
                !is_associative_binary_op(parent_op) || child_op != parent_op
            }
            (RustExpr::Binary { op: child_op, .. }, Some(parent_op), ExprChildSide::Right) => {
                bitwise_binary_op(parent_op) && comparison_binary_op(child_op)
            }
            _ => false,
        }
}

fn normalize_raw_expr_for_context(raw: &str, parent_prec: u8) -> String {
    if parent_prec == 0 {
        strip_redundant_outer_parentheses(raw).to_string()
    } else {
        raw.to_string()
    }
}

fn strip_redundant_outer_parentheses(raw: &str) -> &str {
    let mut trimmed = raw.trim();
    while let Some(inner) = strip_one_balanced_outer_parentheses(trimmed) {
        trimmed = inner.trim();
    }
    trimmed
}

fn strip_one_balanced_outer_parentheses(raw: &str) -> Option<&str> {
    if !raw.starts_with('(') || !raw.ends_with(')') {
        return None;
    }
    let inner = raw[1..raw.len() - 1].trim();
    if inner.is_empty() {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && index != raw.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }
    if depth == 0 {
        Some(&raw[1..raw.len() - 1])
    } else {
        None
    }
}

fn is_unit_expr(expr: &RustExpr) -> bool {
    matches!(expr, RustExpr::Raw(raw) if strip_redundant_outer_parentheses(raw) == "()")
}

fn expr_precedence(expr: &RustExpr) -> u8 {
    match expr {
        RustExpr::Binary { op, .. } => binary_precedence(op),
        RustExpr::Cast { .. } => 6,
        RustExpr::Unary { .. } => 10,
        RustExpr::Field(_, _)
        | RustExpr::Index(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. } => 11,
        RustExpr::Raw(raw) if raw_contains_top_level_cast(raw) => 6,
        RustExpr::Literal(_) | RustExpr::Var(_) | RustExpr::StructLit { .. } | RustExpr::Raw(_) => {
            12
        }
        RustExpr::If { .. } => 1,
    }
}

fn raw_contains_top_level_cast(raw: &str) -> bool {
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth = angle_depth.saturating_sub(1)
            }
            'a' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0
                && raw[idx..].starts_with("as")
                && raw[..idx].ends_with(char::is_whitespace)
                && raw[idx + 2..].starts_with(char::is_whitespace) =>
            {
                return true;
            }
            '"' | '\'' => {
                let quote = ch;
                let mut escaped = false;
                for (_, inner) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == quote {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn binary_precedence(op: &str) -> u8 {
    match op {
        "=" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => 1,
    }
}

fn is_associative_binary_op(op: &str) -> bool {
    matches!(op, "&&" | "||" | "&" | "|" | "^")
}

fn comparison_binary_op(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | "<=" | ">" | ">=")
}

fn bitwise_binary_op(op: &str) -> bool {
    matches!(op, "&" | "|" | "^")
}

fn arithmetic_binary_op(op: &str) -> bool {
    matches!(op, "+" | "-" | "*" | "/" | "%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str) -> RustExpr {
        RustExpr::Var(name.to_string())
    }

    fn binary(op: &str, left: RustExpr, right: RustExpr) -> RustExpr {
        RustExpr::Binary {
            op: op.to_string(),
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[test]
    fn binary_expressions_do_not_emit_outer_parentheses() {
        let expr = binary("&&", binary("<", var("i"), var("n")), var("ready"));

        assert_eq!(emit_expr(&expr), "i < n && ready");
    }

    #[test]
    fn lower_precedence_children_still_get_parentheses() {
        let expr = binary("*", binary("+", var("a"), var("b")), var("c"));

        assert_eq!(emit_expr(&expr), "(a + b) * c");
    }

    #[test]
    fn numeric_binary_chains_preserve_left_grouping() {
        let add = binary("+", binary("+", var("a"), var("b")), var("c"));
        let mul = binary("*", binary("*", var("a"), var("b")), var("c"));

        assert_eq!(emit_expr(&add), "(a + b) + c");
        assert_eq!(emit_expr(&mul), "(a * b) * c");
    }

    #[test]
    fn logical_binary_chains_may_flatten() {
        let expr = binary("&&", binary("&&", var("ready"), var("valid")), var("done"));

        assert_eq!(emit_expr(&expr), "ready && valid && done");
    }

    #[test]
    fn bitwise_boolean_operands_parenthesize_comparisons() {
        let expr = binary(
            "^",
            binary("<", var("direction"), var("d")),
            binary("==", var("sign"), RustExpr::Literal("0i64".to_string())),
        );

        assert_eq!(emit_expr(&expr), "(direction < d) ^ (sign == 0i64)");
    }

    #[test]
    fn unary_expressions_do_not_emit_outer_parentheses() {
        let expr = RustExpr::Unary {
            op: "!".to_string(),
            expr: Box::new(var("done")),
        };

        assert_eq!(emit_expr(&expr), "!done");
    }

    #[test]
    fn casts_drop_statement_position_parentheses_but_keep_binary_operand_parentheses() {
        let cast = RustExpr::Cast {
            expr: Box::new(var("end")),
            ty: "i64".to_string(),
        };

        assert_eq!(emit_expr(&cast), "end as i64");
        assert_eq!(
            emit_expr(&binary("<", cast.clone(), var("limit"))),
            "(end as i64) < limit"
        );
        assert_eq!(
            emit_expr(&binary("*", var("acc"), cast)),
            "acc * (end as i64)"
        );
        assert_eq!(
            emit_expr(&binary(
                "<",
                RustExpr::Raw("values.len() as i32".to_string()),
                var("limit")
            )),
            "(values.len() as i32) < limit"
        );
    }

    #[test]
    fn raw_expressions_drop_redundant_outer_parentheses_only_at_expression_boundary() {
        assert_eq!(
            emit_expr(&RustExpr::Raw(
                "(field_idx < vid_map_p_b.get_fields_count())".to_string()
            )),
            "field_idx < vid_map_p_b.get_fields_count()"
        );
        assert_eq!(
            emit_expr(&RustExpr::Raw(
                "((sample_name_map.is_some() && ready) && !done)".to_string()
            )),
            "(sample_name_map.is_some() && ready) && !done"
        );

        let expr = binary(
            "*",
            RustExpr::Raw("(a + b)".to_string()),
            RustExpr::Var("c".to_string()),
        );
        assert_eq!(emit_expr(&expr), "(a + b) * c");
    }

    #[test]
    fn raw_parentheses_strip_is_balanced_and_string_aware() {
        assert_eq!(emit_expr(&RustExpr::Raw("()".to_string())), "()");
        assert_eq!(emit_expr(&RustExpr::Raw("(())".to_string())), "()");
        assert_eq!(
            emit_expr(&RustExpr::Raw("(path.contains(\"()\"))".to_string())),
            "path.contains(\"()\")"
        );
        assert_eq!(
            emit_expr(&RustExpr::Raw("(left) || (right)".to_string())),
            "(left) || (right)"
        );
    }

    #[test]
    fn statement_position_unit_expressions_are_elided() {
        assert_eq!(
            emit_stmt(&RustStmt::Expr(RustExpr::Raw("()".to_string())), 1),
            ""
        );
        assert_eq!(
            emit_stmt(&RustStmt::Expr(RustExpr::Raw("(())".to_string())), 1),
            ""
        );
    }

    #[test]
    fn return_unit_expr_renders_as_bare_return() {
        assert_eq!(
            emit_stmt(&RustStmt::Return(Some(RustExpr::Raw("()".to_string()))), 1),
            "    return;\n"
        );
        assert_eq!(
            emit_stmt(
                &RustStmt::Return(Some(RustExpr::Raw("Ok(())".to_string()))),
                1
            ),
            "    return Ok(());\n"
        );
    }
}

pub fn emit_type(ty: &RustType) -> String {
    match ty {
        RustType::Unit => "()".to_string(),
        RustType::Bool => "bool".to_string(),
        RustType::I8 => "i8".to_string(),
        RustType::I16 => "i16".to_string(),
        RustType::I32 => "i32".to_string(),
        RustType::I64 => "i64".to_string(),
        RustType::F32 => "f32".to_string(),
        RustType::F64 => "f64".to_string(),
        RustType::Char => "char".to_string(),
        RustType::String => "String".to_string(),
        RustType::Vec(inner) => format!("Vec<{}>", emit_type(inner)),
        RustType::Array2(inner) => format!("ndarray::Array2<{}>", emit_type(inner)),
        RustType::Slice(inner) => format!("[{}]", emit_type(inner)),
        RustType::Option(inner) => format!("Option<{}>", emit_type(inner)),
        RustType::Named(name) => name.clone(),
        RustType::Ref { mutable, inner } => {
            if *mutable {
                format!("&mut {}", emit_type(inner))
            } else {
                format!("&{}", emit_type(inner))
            }
        }
        RustType::Raw(raw) => raw.clone(),
    }
}
