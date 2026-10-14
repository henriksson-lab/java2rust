use crate::rust_ir::*;

use crate::java_naming::{rust_keyword_escape, rust_keyword_is_rawable_identifier};

const JAVA_KEYWORD_LABEL_PREFIX: &str = "__java_keyword_label_";

/// Rust does not permit keyword spellings as loop labels, even where the same
/// spelling is usable as a raw identifier. Java has a different keyword set,
/// so valid labels such as `loop:` need a stable escape shared by the label
/// declaration and every `break`/`continue` that targets it.
fn rust_label_ident(name: &str) -> String {
    if rust_keyword_escape(name).is_some() || name.starts_with(JAVA_KEYWORD_LABEL_PREFIX) {
        format!("{JAVA_KEYWORD_LABEL_PREFIX}{name}")
    } else {
        name.to_string()
    }
}

fn string_value_of_call(function: &RustExpr, args: &[RustExpr]) -> bool {
    if args.len() != 1 {
        return false;
    }
    match function {
        RustExpr::TypeAssociatedPath { ty, member } => {
            member == "value_of"
                && matches!(ty, RustType::Named(name) if rust_string_owner_name(name) == "String")
        }
        RustExpr::Path(path) => {
            matches!(path.as_slice(), [owner, member] if rust_string_owner_name(owner) == "String" && member == "value_of")
        }
        _ => false,
    }
}

fn rust_string_owner_name(owner: &str) -> &str {
    owner.trim_start_matches('<').trim_end_matches('>')
}

fn boxed_any_custom_equals_arg(expr: &RustExpr) -> bool {
    matches!(
        expr,
        RustExpr::Cast { expr, ty }
            if ty == "Box<dyn std::any::Any>" && matches!(
                expr.as_ref(),
                RustExpr::Call { function, args }
                    if args.len() == 1
                        && matches!(function.as_ref(), RustExpr::Path(path)
                            if matches!(path.as_slice(), [owner, member]
                                if owner == "Box" && member == "new"))
            )
    )
}

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
    if strukt.is_unit && strukt.fields.is_empty() {
        out.push_str(&format!("pub struct {};\n", rust_ident(&strukt.name)));
        return out;
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
    let target = rust_ident(&imp.target);
    let mut out = if let Some(trait_name) = &imp.trait_name {
        emit_trait_impl_header(&target, trait_name)
    } else if target.starts_with('<') {
        format!("impl{target} {{\n")
    } else {
        format!("impl {target} {{\n")
    };
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

fn emit_trait_impl_header(target: &str, trait_name: &str) -> String {
    if target.starts_with('<') {
        let mut depth = 0usize;
        for (index, ch) in target.char_indices() {
            match ch {
                '<' => depth += 1,
                '>' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let generics = &target[..=index];
                        let ty = target[index + ch.len_utf8()..].trim_start();
                        if !ty.is_empty() {
                            return format!("impl{generics} {trait_name} for {ty} {{\n");
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    format!("impl {trait_name} for {target} {{\n")
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
    let vis = if function.is_pub {
        "pub "
    } else if function.is_pub_crate {
        "pub(crate) "
    } else {
        ""
    };
    let ret = function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", emit_type(ty)))
        .unwrap_or_default();
    let type_params = if function.type_params.is_empty() {
        String::new()
    } else {
        format!("<{}>", function.type_params.join(", "))
    };
    let where_clause = if function.where_bounds.is_empty() {
        String::new()
    } else {
        format!(" where {}", function.where_bounds.join(", "))
    };
    let mut out = format!(
        "{pad}{vis}fn {}{type_params}({}){ret}{where_clause} {{\n",
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
        RustStmt::AssignOp { target, op, value } => {
            format!("{pad}{} {op} {};\n", emit_expr(target), emit_expr(value))
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
        RustStmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            let mut out = format!("{pad}if let {pattern} = {} {{\n", emit_expr(expr));
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
        RustStmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{}: ", rust_label_ident(l)))
                .unwrap_or_default();
            let mut out = format!("{pad}{label}while let {pattern} = {} {{\n", emit_expr(expr));
            out.push_str(&emit_statements(body, indent + 1));
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        RustStmt::For {
            label,
            pattern,
            iterable,
            body,
        } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{}: ", rust_label_ident(l)))
                .unwrap_or_default();
            let mut out = format!("{pad}{label}for {pattern} in {} {{\n", emit_expr(iterable));
            out.push_str(&emit_statements(body, indent + 1));
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        RustStmt::Loop { label, body } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{}: ", rust_label_ident(l)))
                .unwrap_or_default();
            let mut out = format!("{pad}{label}loop {{\n");
            out.push_str(&emit_statements(body, indent + 1));
            out.push_str(&format!("{pad}}}\n"));
            out
        }
        RustStmt::Break(label) => {
            let label = label
                .as_ref()
                .map(|l| format!(" '{}", rust_label_ident(l)))
                .unwrap_or_default();
            format!("{pad}break{label};\n")
        }
        RustStmt::BreakValue { label, value } => {
            let label = label
                .as_ref()
                .map(|l| format!(" '{}", rust_label_ident(l)))
                .unwrap_or_default();
            format!("{pad}break{label} {};\n", emit_expr(value))
        }
        RustStmt::Continue(label) => {
            let label = label
                .as_ref()
                .map(|l| format!(" '{}", rust_label_ident(l)))
                .unwrap_or_default();
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
        // A shape tag is a note for other passes, never output.
        RustExpr::Shaped { expr, .. } => {
            return emit_expr_prec(expr, parent_prec, parent_op, child_side);
        }
        RustExpr::Literal(value) => value.clone(),
        RustExpr::Var(name) => rust_ident(name),
        RustExpr::Path(segments) => segments
            .iter()
            .map(|segment| rust_ident(segment))
            .collect::<Vec<_>>()
            .join("::"),
        RustExpr::GenericPath { base, args, member } => {
            let base = base
                .iter()
                .map(|segment| rust_ident(segment))
                .collect::<Vec<_>>()
                .join("::");
            let args = args.iter().map(emit_type).collect::<Vec<_>>().join(", ");
            match member {
                Some(member) => format!("{base}::<{args}>::{}", rust_ident(member)),
                None => format!("{base}::<{args}>"),
            }
        }
        RustExpr::TypeAssociatedPath { ty, member } => {
            format!("<{}>::{}", emit_type(ty), rust_ident(member))
        }
        RustExpr::TraitAssociatedPath {
            ty,
            trait_name,
            member,
        } => format!(
            "<{} as {}>::{}",
            emit_type(ty),
            trait_name,
            rust_ident(member)
        ),
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
        RustExpr::Binary { op, left, right }
            if op == "==" && boxed_any_custom_equals_arg(right) =>
        {
            format!(
                "{}.equals({})",
                emit_expr_prec(left, 11, None, ExprChildSide::None),
                emit_expr(right)
            )
        }
        RustExpr::Binary { op, left, right } => format!(
            "{} {op} {}",
            emit_expr_prec(left, binary_precedence(op), Some(op), ExprChildSide::Left),
            emit_expr_prec(right, binary_precedence(op), Some(op), ExprChildSide::Right)
        ),
        RustExpr::Range {
            start,
            end,
            inclusive,
        } => {
            let op = if *inclusive { "..=" } else { ".." };
            format!("{}{}{}", emit_expr(start), op, emit_expr(end))
        }
        RustExpr::Unary { op, expr } => {
            format!(
                "{op}{}",
                emit_expr_prec(expr, 10, None, ExprChildSide::None)
            )
        }
        RustExpr::Cast { expr, ty } => {
            let rendered = emit_expr_prec(expr, 11, None, ExprChildSide::None);
            // A Rust integer literal takes its type from the cast target, so
            // `(byte) 200` rendered as `200 as i8` is rejected as out of range
            // and `(-1) as u32` negates an unsigned value. Java always casts a
            // value that already has a wider type, so pin the literal and let
            // the cast truncate — which is the Java narrowing.
            let rendered = pin_integer_literal_before_cast(rendered, ty);
            format!("{rendered} as {ty}")
        }
        RustExpr::Try(expr) => {
            format!("{}?", emit_expr_prec(expr, 11, None, ExprChildSide::None))
        }
        RustExpr::Grouped(expr) => format!("({})", emit_expr(expr)),
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
        RustExpr::Tuple(items) => match items.as_slice() {
            [] => "()".to_string(),
            [item] => format!("({},)", emit_expr(item)),
            _ => format!(
                "({})",
                items.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
            ),
        },
        RustExpr::ArrayLiteral(items) => format!(
            "[{}]",
            items.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::VecLiteral(items) => format!(
            "vec![{}]",
            items.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::VecRepeat { value, len } => {
            format!("vec![{}; {}]", emit_expr(value), emit_expr(len))
        }
        RustExpr::Call { function, args } if string_value_of_call(function, args) => {
            format!("({}).to_string()", emit_expr(&args[0]))
        }
        RustExpr::Call { function, args } => format!(
            "{}({})",
            emit_expr_prec(function, 11, None, ExprChildSide::None),
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::MacroCall { macro_name, args } => format!(
            "{}!({})",
            rust_ident(macro_name),
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            if method == "as_str"
                && args.is_empty()
                && let Some(literal) = string_literal_to_string_receiver(receiver)
            {
                literal
            } else {
                format!(
                    "{}.{}({})",
                    emit_expr_prec(receiver, 11, None, ExprChildSide::None),
                    rust_ident(method),
                    args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
                )
            }
        }
        RustExpr::Closure {
            is_move,
            params,
            return_type,
            body,
        } => emit_closure_expr(*is_move, params, return_type.as_ref(), body),
        RustExpr::Pattern(pattern) => emit_pattern(pattern),
        RustExpr::Match { expr, arms } => emit_match_expr(expr, arms),
        RustExpr::Block { stmts, tail } => emit_block_expr(stmts, tail.as_deref()),
        RustExpr::StructLit { name, fields } => {
            let fields = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", rust_ident(name), emit_expr(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} {{ {fields} }}", rust_struct_literal_path(name))
        }
        RustExpr::PostfixComment { expr, comment } => {
            format!(
                "{}{comment}",
                emit_expr_prec(expr, 0, None, ExprChildSide::None)
            )
        }
        RustExpr::UnsupportedJavaArrayCreation {
            element_type,
            dimensions,
        } => {
            let dims = dimensions
                .iter()
                .map(emit_expr)
                .collect::<Vec<_>>()
                .join("][");
            format!(
                "Vec::new() /* {}: {element_type}[{dims}] */",
                crate::output_markers::UNSUPPORTED_JAVA_ARRAY_CREATION
            )
        }
        RustExpr::MarkedBlock { marker, expr } => {
            format!("{{ {marker} {} }}", emit_expr(expr))
        }
        RustExpr::Raw(raw) => normalize_raw_expr_for_context(raw, parent_prec),
    };

    if expr_needs_parentheses(expr, parent_prec, parent_op, child_side) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn string_literal_to_string_receiver(receiver: &RustExpr) -> Option<String> {
    match receiver {
        RustExpr::Shaped { expr, .. } => string_literal_to_string_receiver(expr),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "to_string" && args.is_empty() => match receiver.as_ref() {
            RustExpr::Literal(literal) if is_complete_string_literal(literal) => {
                Some(literal.clone())
            }
            _ => None,
        },
        RustExpr::Literal(rendered) => {
            let literal = rendered.strip_suffix(".to_string()")?;
            is_complete_string_literal(literal).then(|| literal.to_string())
        }
        _ => None,
    }
}

fn is_complete_string_literal(rendered: &str) -> bool {
    if !rendered.starts_with('"') {
        return false;
    }
    let mut escaped = false;
    for (idx, ch) in rendered.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return idx + ch.len_utf8() == rendered.len();
        }
    }
    false
}

fn emit_closure_expr(
    is_move: bool,
    params: &[crate::rust_ir::RustClosureParam],
    return_type: Option<&RustType>,
    body: &crate::rust_ir::RustClosureBody,
) -> String {
    let move_prefix = if is_move { "move " } else { "" };
    let params = params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {ty}", param.pattern),
            None => param.pattern.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    match return_type {
        Some(return_type) => format!(
            "{move_prefix}|{params}| -> {} {}",
            emit_type(return_type),
            emit_typed_closure_body(body)
        ),
        None => {
            let body = match body {
                crate::rust_ir::RustClosureBody::Expr(expr) => emit_expr(expr),
            };
            format!("{move_prefix}|{params}| {body}")
        }
    }
}

fn emit_typed_closure_body(body: &crate::rust_ir::RustClosureBody) -> String {
    match body {
        crate::rust_ir::RustClosureBody::Expr(expr) => match expr.as_ref() {
            RustExpr::Block { stmts, tail } => emit_block_expr(stmts, tail.as_deref()),
            _ => format!("{{ {} }}", emit_expr(expr)),
        },
    }
}

fn emit_pattern(pattern: &RustPattern) -> String {
    match pattern {
        RustPattern::TupleVariantWildcard {
            enum_name,
            variant_name,
        } => {
            format!("{enum_name}::{variant_name}(_)")
        }
    }
}

fn emit_match_expr(expr: &RustExpr, arms: &[crate::rust_ir::RustMatchArm]) -> String {
    let arms = arms
        .iter()
        .map(|arm| format!("{} => {}", arm.pattern, emit_expr(&arm.body)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("match {} {{ {arms} }}", emit_expr(expr))
}

fn emit_block_expr(stmts: &[RustStmt], tail: Option<&RustExpr>) -> String {
    let mut parts = stmts
        .iter()
        .map(emit_stmt_as_block_part)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if let Some(tail) = tail {
        parts.push(emit_expr(tail));
    }
    if parts.is_empty() {
        "{ }".to_string()
    } else {
        format!("{{ {} }}", parts.join(" "))
    }
}

fn emit_stmt_as_block_part(stmt: &RustStmt) -> String {
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
            format!("let {mutability}{}{ty}{value};", rust_ident(name))
        }
        RustStmt::Assign { target, value } => {
            format!("{} = {};", emit_expr(target), emit_expr(value))
        }
        RustStmt::AssignOp { target, op, value } => {
            format!("{} {op} {};", emit_expr(target), emit_expr(value))
        }
        RustStmt::Expr(expr) if is_unit_expr(expr) => String::new(),
        RustStmt::Expr(expr) => format!("{};", emit_expr(expr)),
        RustStmt::Return(Some(expr)) if is_unit_expr(expr) => "return;".to_string(),
        RustStmt::Return(Some(expr)) => format!("return {};", emit_expr(expr)),
        RustStmt::Return(None) => "return;".to_string(),
        RustStmt::BreakValue { label, value } => {
            let label = label
                .as_ref()
                .map(|l| format!(" '{}", rust_label_ident(l)))
                .unwrap_or_default();
            format!("break{label} {};", emit_expr(value))
        }
        RustStmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let then_body = then_body
                .iter()
                .map(emit_stmt_as_block_part)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let mut rendered = format!("if {} {{ {then_body} }}", emit_expr(condition));
            if !else_body.is_empty() {
                let else_body = else_body
                    .iter()
                    .map(emit_stmt_as_block_part)
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                rendered.push_str(&format!(" else {{ {else_body} }}"));
            }
            rendered
        }
        RustStmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            let then_body = then_body
                .iter()
                .map(emit_stmt_as_block_part)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let mut rendered = format!("if let {pattern} = {} {{ {then_body} }}", emit_expr(expr));
            if !else_body.is_empty() {
                let else_body = else_body
                    .iter()
                    .map(emit_stmt_as_block_part)
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                rendered.push_str(&format!(" else {{ {else_body} }}"));
            }
            rendered
        }
        RustStmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{}: ", rust_label_ident(l)))
                .unwrap_or_default();
            let body = body
                .iter()
                .map(emit_stmt_as_block_part)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "{label}while let {pattern} = {} {{ {body} }}",
                emit_expr(expr)
            )
        }
        RustStmt::For {
            label,
            pattern,
            iterable,
            body,
        } => {
            let label = label
                .as_ref()
                .map(|l| format!("'{}: ", rust_label_ident(l)))
                .unwrap_or_default();
            let body = body
                .iter()
                .map(emit_stmt_as_block_part)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "{label}for {pattern} in {} {{ {body} }}",
                emit_expr(iterable)
            )
        }
        other => emit_stmt(other, 0).trim().to_string(),
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
    if rust_keyword_is_rawable_identifier(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn rust_struct_literal_path(name: &str) -> String {
    let name = rust_ident(name);
    if name.contains("::<") {
        return name;
    }
    let Some(generic_start) = name.find('<') else {
        return name;
    };
    let mut rendered = name;
    rendered.insert_str(generic_start, "::");
    rendered
}

fn rust_param_ident(name: &str) -> String {
    if let Some(inner) = name.strip_prefix("mut ") {
        return format!("mut {}", rust_ident(inner));
    }
    rust_ident(name)
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
    if let Some(rendered) = normalize_raw_optional_string_literal_view(raw) {
        rendered
    } else if parent_prec == 0 {
        strip_redundant_outer_parentheses(raw).to_string()
    } else {
        raw.to_string()
    }
}

fn normalize_raw_optional_string_literal_view(raw: &str) -> Option<String> {
    let inner = raw
        .trim()
        .strip_prefix("Some(")?
        .strip_suffix(".to_string().as_str())")?;
    is_complete_string_literal(inner).then(|| format!("Some({inner})"))
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
    let mut skip_until = 0usize;
    for (index, ch) in raw.char_indices() {
        if index < skip_until {
            continue;
        }
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
        if let Some(end) = raw_string_literal_end_at(raw, index) {
            skip_until = end;
            continue;
        }
        if raw[index..].starts_with("//") {
            skip_until = raw[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(raw.len());
            continue;
        }
        if raw[index..].starts_with("/*") {
            skip_until = raw[index + 2..]
                .find("*/")
                .map(|offset| index + 2 + offset + 2)
                .unwrap_or(raw.len());
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
    match expr {
        RustExpr::Shaped { expr, .. } => is_unit_expr(expr),
        RustExpr::PostfixComment { expr, .. } => is_unit_expr(expr),
        RustExpr::UnsupportedJavaArrayCreation { .. } => false,
        RustExpr::Raw(raw) => strip_redundant_outer_parentheses(raw) == "()",
        _ => false,
    }
}

pub(crate) fn expr_precedence(expr: &RustExpr) -> u8 {
    match expr {
        RustExpr::Shaped { expr, .. } => expr_precedence(expr),
        RustExpr::PostfixComment { expr, .. } => expr_precedence(expr),
        RustExpr::UnsupportedJavaArrayCreation { .. } => 12,
        RustExpr::Binary { op, .. } => binary_precedence(op),
        RustExpr::Range { .. } => 2,
        RustExpr::Cast { .. } => 6,
        RustExpr::Try(_) => 11,
        RustExpr::Grouped(_) => 12,
        RustExpr::Unary { .. } => 10,
        RustExpr::Field(_, _)
        | RustExpr::Index(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MacroCall { .. }
        | RustExpr::MethodCall { .. } => 11,
        RustExpr::Raw(raw) if raw_contains_top_level_cast(raw) => 6,
        RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::TraitAssociatedPath { .. }
        | RustExpr::Pattern(_)
        | RustExpr::Tuple(_)
        | RustExpr::ArrayLiteral(_)
        | RustExpr::VecLiteral(_)
        | RustExpr::VecRepeat { .. }
        | RustExpr::Closure { .. }
        | RustExpr::Block { .. }
        | RustExpr::StructLit { .. }
        | RustExpr::MarkedBlock { .. }
        | RustExpr::Raw(_) => 12,
        RustExpr::If { .. } | RustExpr::Match { .. } => 1,
    }
}

fn raw_contains_top_level_cast(raw: &str) -> bool {
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut skip_until = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if idx < skip_until {
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = end;
            continue;
        }
        if raw[idx..].starts_with("//") {
            skip_until = raw[idx..]
                .find('\n')
                .map(|offset| idx + offset)
                .unwrap_or(raw.len());
            continue;
        }
        if raw[idx..].starts_with("/*") {
            skip_until = raw[idx + 2..]
                .find("*/")
                .map(|offset| idx + 2 + offset + 2)
                .unwrap_or(raw.len());
            continue;
        }
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

fn raw_string_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first != 'r' {
        return None;
    }
    let mut hashes = 0usize;
    while let Some((_, '#')) = chars.clone().next() {
        hashes += 1;
        chars.next();
    }
    let (quote_offset, quote) = chars.next()?;
    if quote != '"' {
        return None;
    }
    let terminator = format!("\"{}", "#".repeat(hashes));
    let body_start = idx + quote_offset + 1;
    raw.get(body_start..)?
        .find(&terminator)
        .map(|end| body_start + end + terminator.len())
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

    fn empty_impl(target: &str, trait_name: Option<&str>) -> RustItem {
        RustItem::Impl(RustImpl {
            target: target.to_string(),
            trait_name: trait_name.map(str::to_string),
            associated_consts: Vec::new(),
            functions: Vec::new(),
        })
    }

    #[test]
    fn structured_impl_items_emit_trait_headers() {
        assert_eq!(
            emit_item(&empty_impl("Target", Some("From<Source>"))),
            "impl From<Source> for Target {\n}\n"
        );
        assert_eq!(
            emit_item(&empty_impl("<T> Target<T>", Some("From<Source<T>>"))),
            "impl<T> From<Source<T>> for Target<T> {\n}\n"
        );
        assert_eq!(
            emit_item(&empty_impl(
                "<K, V: Into<Pair<A, B>>> Target<K, V>",
                Some("From<Source<K, V>>"),
            )),
            "impl<K, V: Into<Pair<A, B>>> From<Source<K, V>> for Target<K, V> {\n}\n"
        );
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
    fn grouped_expressions_emit_explicit_parentheses() {
        let expr = RustExpr::Grouped(Box::new(binary("+", var("a"), var("b"))));

        assert_eq!(emit_expr(&expr), "(a + b)");
        assert_eq!(emit_expr(&binary("*", expr, var("c"))), "(a + b) * c");
        assert_eq!(
            emit_expr(&RustExpr::Range {
                start: Box::new(var("start")),
                end: Box::new(var("end")),
                inclusive: false,
            }),
            "start..end"
        );
        assert_eq!(
            emit_expr(&RustExpr::Range {
                start: Box::new(var("start")),
                end: Box::new(var("end")),
                inclusive: true,
            }),
            "start..=end"
        );
    }

    #[test]
    fn raw_optional_string_literal_view_normalization_requires_complete_literal() {
        assert!(is_complete_string_literal("\"a \\\"quoted\\\" value\""));
        assert!(!is_complete_string_literal("\"a\" garbage \""));
        assert_eq!(
            emit_expr(&RustExpr::Raw(
                "Some(\"a\" garbage \".to_string().as_str())".to_string()
            )),
            "Some(\"a\" garbage \".to_string().as_str())"
        );
        assert_eq!(
            emit_expr(&RustExpr::Raw(
                "Some(\"actual\".to_string().as_str())".to_string()
            )),
            "Some(\"actual\")"
        );
    }

    #[test]
    fn string_literal_to_string_as_str_emits_static_literal_view() {
        let structured = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Literal("\"none\"".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }),
            method: "as_str".to_string(),
            args: Vec::new(),
        };
        assert_eq!(emit_expr(&structured), "\"none\"");

        let raw_receiver = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("\"none\".to_string()".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        };
        assert_eq!(emit_expr(&raw_receiver), "\"none\".to_string().as_str()");

        let literal_receiver = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Literal("\"none\".to_string()".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        };
        assert_eq!(emit_expr(&literal_receiver), "\"none\"");

        let raw_optional = RustExpr::Raw("Some(\"none\".to_string().as_str())".to_string());
        assert_eq!(emit_expr(&raw_optional), "Some(\"none\")");
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
    fn try_expressions_parenthesize_only_when_needed() {
        assert_eq!(emit_expr(&RustExpr::Try(Box::new(var("call")))), "call?");
        assert_eq!(
            emit_expr(&RustExpr::Try(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(var("left")),
                right: Box::new(var("right")),
            }))),
            "(left + right)?"
        );
    }

    #[test]
    fn tuple_expressions_keep_required_parentheses() {
        assert_eq!(emit_expr(&RustExpr::Tuple(Vec::new())), "()");
        assert_eq!(emit_expr(&RustExpr::Tuple(vec![var("only")])), "(only,)");
        assert_eq!(
            emit_expr(&RustExpr::Tuple(vec![
                RustExpr::Cast {
                    expr: Box::new(var("rows")),
                    ty: "usize".to_string(),
                },
                RustExpr::Cast {
                    expr: Box::new(var("cols")),
                    ty: "usize".to_string(),
                },
            ])),
            "(rows as usize, cols as usize)"
        );
    }

    #[test]
    fn array_literal_expressions_render_bracketed_items() {
        assert_eq!(emit_expr(&RustExpr::ArrayLiteral(Vec::new())), "[]");
        assert_eq!(
            emit_expr(&RustExpr::ArrayLiteral(vec![
                RustExpr::Literal("1".to_string()),
                RustExpr::Tuple(vec![var("key"), var("value")]),
            ])),
            "[1, (key, value)]"
        );
    }

    #[test]
    fn vec_macro_expressions_render_literal_and_repeat_forms() {
        assert_eq!(emit_expr(&RustExpr::VecLiteral(Vec::new())), "vec![]");
        assert_eq!(
            emit_expr(&RustExpr::VecLiteral(vec![
                RustExpr::Literal("1".to_string()),
                RustExpr::Literal("2".to_string()),
            ])),
            "vec![1, 2]"
        );
        assert_eq!(
            emit_expr(&RustExpr::VecRepeat {
                value: Box::new(RustExpr::Literal("0".to_string())),
                len: Box::new(RustExpr::Cast {
                    expr: Box::new(var("len")),
                    ty: "usize".to_string(),
                }),
            }),
            "vec![0; len as usize]"
        );
    }

    #[test]
    fn closure_expressions_emit_move_and_typed_params() {
        assert_eq!(
            emit_expr(&RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: vec![crate::rust_ir::RustClosureParam {
                    pattern: "value".to_string(),
                    ty: None,
                }],
                body: crate::rust_ir::RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "as_str".to_string(),
                    args: Vec::new(),
                })),
            }),
            "|value| value.as_str()"
        );
        assert_eq!(
            emit_expr(&RustExpr::Closure {
                is_move: true,
                return_type: None,
                params: vec![crate::rust_ir::RustClosureParam {
                    pattern: "(k, v)".to_string(),
                    ty: Some("(String, i32)".to_string()),
                }],
                body: crate::rust_ir::RustClosureBody::Expr(Box::new(RustExpr::Tuple(vec![
                    RustExpr::Var("k".to_string()),
                    RustExpr::Var("v".to_string()),
                ]))),
            }),
            "move |(k, v): (String, i32)| (k, v)"
        );
        assert_eq!(
            emit_expr(&RustExpr::Closure {
                is_move: false,
                return_type: Some(RustType::Generic {
                    base: "Result".to_string(),
                    args: vec![RustType::Unit, RustType::Named("JavaException".to_string())],
                }),
                params: Vec::new(),
                body: crate::rust_ir::RustClosureBody::Expr(Box::new(RustExpr::Block {
                    stmts: vec![RustStmt::Return(Some(RustExpr::Call {
                        function: Box::new(RustExpr::path(["Ok"])),
                        args: vec![RustExpr::Tuple(Vec::new())],
                    }))],
                    tail: None,
                })),
            }),
            "|| -> Result<(), JavaException> { return Ok(()); }"
        );
    }

    #[test]
    fn match_expressions_emit_structured_scrutinee_and_arms() {
        assert_eq!(
            emit_expr(&RustExpr::Match {
                expr: Box::new(RustExpr::Tuple(vec![var("left"), var("right")])),
                arms: vec![
                    crate::rust_ir::RustMatchArm {
                        pattern: "(Some(left), Some(right))".to_string(),
                        body: RustExpr::MethodCall {
                            receiver: Box::new(var("left")),
                            method: "equals".to_string(),
                            args: vec![var("right")],
                        },
                    },
                    crate::rust_ir::RustMatchArm {
                        pattern: "_".to_string(),
                        body: RustExpr::Literal("false".to_string()),
                    },
                ],
            }),
            "match (left, right) { (Some(left), Some(right)) => left.equals(right), _ => false }"
        );
    }

    #[test]
    fn while_let_statements_emit_label_pattern_and_body() {
        assert_eq!(
            emit_stmt(
                &RustStmt::WhileLet {
                    label: Some("scan".to_string()),
                    pattern: "Some(item)".to_string(),
                    expr: RustExpr::MethodCall {
                        receiver: Box::new(var("cursor")),
                        method: "next".to_string(),
                        args: Vec::new(),
                    },
                    body: vec![RustStmt::Expr(RustExpr::MethodCall {
                        receiver: Box::new(var("items")),
                        method: "push".to_string(),
                        args: vec![var("item")],
                    })],
                },
                1,
            ),
            "    'scan: while let Some(item) = cursor.next() {\n        items.push(item);\n    }\n"
        );
    }

    #[test]
    fn for_statements_emit_label_pattern_iterable_and_body() {
        assert_eq!(
            emit_stmt(
                &RustStmt::For {
                    label: Some("items".to_string()),
                    pattern: "mut item".to_string(),
                    iterable: RustExpr::MethodCall {
                        receiver: Box::new(var("values")),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    },
                    body: vec![RustStmt::Expr(RustExpr::MethodCall {
                        receiver: Box::new(var("item")),
                        method: "touch".to_string(),
                        args: Vec::new(),
                    })],
                },
                1,
            ),
            "    'items: for mut item in values.iter() {\n        item.touch();\n    }\n"
        );
    }

    #[test]
    fn rust_keyword_loop_labels_use_one_stable_escape() {
        assert_eq!(
            emit_stmt(
                &RustStmt::Loop {
                    label: Some("loop".to_string()),
                    body: vec![
                        RustStmt::Continue(Some("loop".to_string())),
                        RustStmt::Break(Some("loop".to_string())),
                    ],
                },
                0,
            ),
            "'__java_keyword_label_loop: loop {\n    continue '__java_keyword_label_loop;\n    break '__java_keyword_label_loop;\n}\n"
        );
    }

    #[test]
    fn block_expressions_emit_statements_and_tail() {
        assert_eq!(
            emit_expr(&RustExpr::Block {
                stmts: vec![
                    RustStmt::Let {
                        mutable: true,
                        name: "items".to_string(),
                        ty: None,
                        value: Some(var("seed")),
                    },
                    RustStmt::Expr(RustExpr::MethodCall {
                        receiver: Box::new(var("items")),
                        method: "extend".to_string(),
                        args: vec![var("tail")],
                    }),
                ],
                tail: Some(Box::new(var("items"))),
            }),
            "{ let mut items = seed; items.extend(tail); items }"
        );
        assert_eq!(
            emit_expr(&RustExpr::Block {
                stmts: vec![RustStmt::Let {
                    mutable: false,
                    name: "_".to_string(),
                    ty: None,
                    value: Some(var("value")),
                }],
                tail: None,
            }),
            "{ let _ = value; }"
        );
    }

    #[test]
    fn ref_types_emit_optional_lifetimes() {
        assert_eq!(
            emit_type(&RustType::Ref {
                mutable: false,
                lifetime: Some("'static".to_string()),
                inner: Box::new(RustType::Slice(Box::new(RustType::I32))),
            }),
            "&'static [i32]"
        );
        assert_eq!(
            emit_type(&RustType::Ref {
                mutable: true,
                lifetime: Some("'a".to_string()),
                inner: Box::new(RustType::String),
            }),
            "&'a mut String"
        );
    }

    #[test]
    fn dyn_and_impl_trait_types_emit_without_raw_type_strings() {
        assert_eq!(
            emit_type(&RustType::DynTrait("std::any::Any".to_string())),
            "dyn std::any::Any"
        );
        assert_eq!(
            emit_type(&RustType::ImplTrait("dep_facade::Locatable".to_string())),
            "impl dep_facade::Locatable"
        );
        assert_eq!(
            emit_type(&RustType::Generic {
                base: "Box".to_string(),
                args: vec![RustType::DynTrait("Animal".to_string())],
            }),
            "Box<dyn Animal>"
        );
        assert_eq!(
            emit_type(&RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![],
                return_type: None,
            }),
            "impl FnMut()"
        );
        assert_eq!(
            emit_type(&RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![RustType::String, RustType::I32],
                return_type: None,
            }),
            "impl FnMut(String, i32)"
        );
        assert_eq!(
            emit_type(&RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![],
                return_type: Some(Box::new(RustType::String)),
            }),
            "impl FnMut() -> String"
        );
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
            emit_expr(&RustExpr::Raw("(value /* ) */)".to_string())),
            "value /* ) */"
        );
        assert_eq!(
            emit_expr(&RustExpr::Raw("(value // )\n)".to_string())),
            "value // )"
        );
        assert_eq!(
            emit_expr(&RustExpr::Raw("(format!(r#\"(\"#))".to_string())),
            "format!(r#\"(\"#)"
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
        assert_eq!(
            emit_stmt(
                &RustStmt::Expr(RustExpr::shaped(
                    RustExprShape::ResultPropagated,
                    RustExpr::Raw("()".to_string())
                )),
                1
            ),
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
        assert_eq!(
            emit_stmt(
                &RustStmt::Return(Some(RustExpr::shaped(
                    RustExprShape::ResultPropagated,
                    RustExpr::Raw("()".to_string())
                ))),
                1
            ),
            "    return;\n"
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
        RustType::Str => "str".to_string(),
        RustType::Vec(inner) => format!("Vec<{}>", emit_type(inner)),
        RustType::Array2(inner) => format!("ndarray::Array2<{}>", emit_type(inner)),
        RustType::Slice(inner) => format!("[{}]", emit_type(inner)),
        RustType::Option(inner) => format!("Option<{}>", emit_type(inner)),
        RustType::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter().map(emit_type).collect::<Vec<_>>().join(", ")
        ),
        RustType::Tuple(items) => {
            let items = items.iter().map(emit_type).collect::<Vec<_>>().join(", ");
            format!("({items})")
        }
        RustType::FnPointer {
            params,
            return_type,
        } => {
            let params = params.iter().map(emit_type).collect::<Vec<_>>().join(", ");
            format!("fn({params}) -> {}", emit_type(return_type))
        }
        RustType::ImplFnTrait {
            trait_name,
            params,
            return_type,
        } => {
            let params = params.iter().map(emit_type).collect::<Vec<_>>().join(", ");
            match return_type {
                Some(return_type) => {
                    format!("impl {trait_name}({params}) -> {}", emit_type(return_type))
                }
                None => format!("impl {trait_name}({params})"),
            }
        }
        RustType::DynTrait(trait_name) => format!("dyn {trait_name}"),
        RustType::ImplTrait(trait_name) => format!("impl {trait_name}"),
        RustType::Named(name) => name.clone(),
        RustType::Ref {
            mutable,
            lifetime,
            inner,
        } => match (lifetime.as_deref(), mutable) {
            (Some(lifetime), true) => format!("&{lifetime} mut {}", emit_type(inner)),
            (Some(lifetime), false) => format!("&{lifetime} {}", emit_type(inner)),
            (None, true) => format!("&mut {}", emit_type(inner)),
            (None, false) => format!("&{}", emit_type(inner)),
        },
        RustType::Raw(raw) => raw.clone(),
    }
}

/// Give a bare integer literal a source type before a narrowing cast — but only
/// when it needs one.
///
/// `200 as i8` is rejected outright: rustc infers the literal's type *from the
/// cast*, and 200 does not fit `i8`. `200i64 as i8` truncates instead, which is
/// what Java does. The same inference is why `-1 as u32` computed the wrong
/// value — the literal became unsigned and was then negated.
///
/// `1 as i8` needs no help. The test is therefore "does this literal fit the
/// target", not "is the target narrow": pinning every literal would compile
/// just as correctly but make the output noisier for no reason.
fn pin_integer_literal_before_cast(rendered: String, target: &str) -> String {
    let Some(bounds) = integer_target_bounds(target) else {
        return rendered;
    };
    let Ok(value) = rendered.parse::<i128>() else {
        return rendered;
    };
    if bounds.contains(&value) {
        return rendered;
    }
    format!("{rendered}i64")
}

/// The value range of a Rust integer type, for deciding whether a literal can
/// be left unsuffixed. `usize`/`isize` are taken as 64-bit; on a narrower target
/// the answer is only ever more conservative, which costs a suffix rather than
/// correctness.
fn integer_target_bounds(target: &str) -> Option<std::ops::RangeInclusive<i128>> {
    Some(match target {
        "i8" => i128::from(i8::MIN)..=i128::from(i8::MAX),
        "i16" => i128::from(i16::MIN)..=i128::from(i16::MAX),
        "i32" => i128::from(i32::MIN)..=i128::from(i32::MAX),
        "u8" => 0..=i128::from(u8::MAX),
        "u16" => 0..=i128::from(u16::MAX),
        "u32" => 0..=i128::from(u32::MAX),
        "usize" => 0..=i128::from(u64::MAX),
        "isize" => i128::from(i64::MIN)..=i128::from(i64::MAX),
        _ => return None,
    })
}
