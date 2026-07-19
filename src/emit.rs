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
    let mut out = format!("pub struct {} {{\n", strukt.name);
    for field in &strukt.fields {
        let vis = if field.is_pub { "pub " } else { "" };
        out.push_str(&format!(
            "    {vis}{}: {},\n",
            field.name,
            emit_type(&field.ty)
        ));
    }
    out.push_str("}\n");
    out
}

fn emit_impl(imp: &RustImpl) -> String {
    let mut out = format!("impl {} {{\n", imp.target);
    for associated_const in &imp.associated_consts {
        let vis = if associated_const.is_pub { "pub " } else { "" };
        out.push_str(&format!(
            "    {vis}const {}: {} = {};\n\n",
            associated_const.name,
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
            .map(|p| format!("{}: {}", p.name, emit_type(&p.ty))),
    );
    let vis = if function.is_pub { "pub " } else { "" };
    let ret = function
        .return_type
        .as_ref()
        .map(|ty| format!(" -> {}", emit_type(ty)))
        .unwrap_or_default();
    let mut out = format!(
        "{pad}{vis}fn {}({}){ret} {{\n",
        function.name,
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
            format!("{pad}let {mutability}{name}{ty}{value};\n")
        }
        RustStmt::Assign { target, value } => {
            format!("{pad}{} = {};\n", emit_expr(target), emit_expr(value))
        }
        RustStmt::Expr(expr) => format!("{pad}{};\n", emit_expr(expr)),
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
    match expr {
        RustExpr::Literal(value) => value.clone(),
        RustExpr::Var(name) => name.clone(),
        RustExpr::Field(receiver, field) => format!("{}.{}", emit_expr(receiver), field),
        RustExpr::Index(array, index) => format!("{}[{}]", emit_expr(array), emit_expr(index)),
        RustExpr::Binary { op, left, right } => {
            format!("({} {op} {})", emit_expr(left), emit_expr(right))
        }
        RustExpr::Unary { op, expr } => format!("({op}{})", emit_expr(expr)),
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => format!(
            "if {} {{ {} }} else {{ {} }}",
            emit_expr(condition),
            emit_expr(then_expr),
            emit_expr(else_expr)
        ),
        RustExpr::Call { function, args } => format!(
            "{}({})",
            emit_expr(function),
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => format!(
            "{}.{}({})",
            emit_expr(receiver),
            method,
            args.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
        RustExpr::StructLit { name, fields } => {
            let fields = fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", emit_expr(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name} {{ {fields} }}")
        }
        RustExpr::Raw(raw) => raw.clone(),
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
