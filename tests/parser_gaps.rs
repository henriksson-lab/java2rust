use new_java2rust::java_ir::{
    BinaryOp, Expr, JavaType, Literal, Member, PrimitiveType, Stmt, TypeKind, UnaryOp,
};
use new_java2rust::parser::parse_java_source;

fn parser_gap(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new("examples/parser-gaps").join(path)).unwrap()
}

#[test]
fn parses_array_initializer_literals_without_raw_fallbacks() {
    let parsed = parse_java_source(&parser_gap("ArrayInitializerLiteral.java"));
    assert!(collect_program_fallbacks(&parsed.program).is_empty());

    let one_dimensional = method_body(&parsed.program.types[0], "oneDimensional");
    let Stmt::VarDecl {
        initializer: Some(Expr::ArrayInitializer(elements)),
        ..
    } = &one_dimensional.statements[0]
    else {
        panic!("expected one-dimensional array initializer");
    };
    assert_eq!(elements.len(), 6);
    assert!(matches!(
        &elements[2],
        Expr::Unary {
            op: UnaryOp::Pos,
            expr
        } if matches!(expr.as_ref(), Expr::Literal(Literal::Double(2.0)))
    ));

    let two_dimensional = method_body(&parsed.program.types[0], "twoDimensional");
    let Stmt::VarDecl {
        initializer: Some(Expr::ArrayInitializer(rows)),
        ..
    } = &two_dimensional.statements[0]
    else {
        panic!("expected two-dimensional array initializer");
    };
    assert_eq!(rows.len(), 2);
    assert!(matches!(&rows[0], Expr::ArrayInitializer(row) if row.len() == 3));
    assert!(matches!(&rows[1], Expr::ArrayInitializer(row) if row.len() == 3));
}

#[test]
fn parses_array_creation_and_unary_plus_without_raw_fallbacks() {
    let parsed = parse_java_source(&parser_gap("ArrayCreationAndUnaryPlus.java"));
    assert!(collect_program_fallbacks(&parsed.program).is_empty());

    let unary_plus = method_body(&parsed.program.types[0], "unaryPlus");
    assert!(matches!(
        &unary_plus.statements[0],
        Stmt::Return(Some(Expr::Unary {
            op: UnaryOp::Pos,
            expr
        })) if matches!(expr.as_ref(), Expr::Literal(Literal::Double(2.0)))
    ));

    let one_dimensional = method_body(&parsed.program.types[0], "oneDimensional");
    let Stmt::VarDecl {
        initializer:
            Some(Expr::ArrayCreation {
                element_type,
                dimensions,
                initializer,
            }),
        ..
    } = &one_dimensional.statements[0]
    else {
        panic!("expected one-dimensional array creation");
    };
    assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Double));
    assert_eq!(dimensions.len(), 1);
    assert_eq!(initializer, &None);

    let two_dimensional = method_body(&parsed.program.types[0], "twoDimensional");
    let Stmt::VarDecl {
        initializer:
            Some(Expr::ArrayCreation {
                element_type,
                dimensions,
                initializer,
            }),
        ..
    } = &two_dimensional.statements[0]
    else {
        panic!("expected two-dimensional array creation");
    };
    assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Double));
    assert_eq!(dimensions.len(), 2);
    assert_eq!(initializer, &None);
}

#[test]
fn parses_nested_qualified_constructor_type_from_object_creation_prefix() {
    let parsed = parse_java_source(
        r#"
import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import net.sourceforge.iharder.base64.Base64;

class NestedQualifiedConstructor {
    OutputStream wrap(ByteArrayOutputStream os) {
        return new Base64.OutputStream(os);
    }
}
"#,
    );
    let body = method_body(&parsed.program.types[0], "wrap");
    let Stmt::Return(Some(Expr::New { ty, .. })) = &body.statements[0] else {
        panic!("expected nested constructor return");
    };
    assert_eq!(
        ty,
        &JavaType::Named {
            name: "Base64.OutputStream".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn preserves_explicit_java_io_stream_parameter_type_names() {
    let parsed = parse_java_source(
        r#"
class ExplicitJavaIoStreamParam {
    ExplicitJavaIoStreamParam(java.io.InputStream in, java.io.OutputStream out) {}
}
"#,
    );
    let ctor = parsed.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .expect("constructor");

    assert_eq!(
        ctor.params[0].ty,
        JavaType::Named {
            name: "java.io.InputStream".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        ctor.params[1].ty,
        JavaType::Named {
            name: "java.io.OutputStream".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn parses_local_array_dimensions_between_type_and_name() {
    let parsed = parse_java_source(
        r#"
class LocalArrayDimensionSyntax {
    void read() {
        byte [] array = new byte[128];
    }
}
"#,
    );
    let body = method_body(&parsed.program.types[0], "read");
    let Stmt::VarDecl { ty, .. } = &body.statements[0] else {
        panic!("expected local array declaration");
    };
    assert_eq!(
        ty,
        &JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)))
    );
}

#[test]
fn local_array_dimensions_ignore_generic_argument_arrays() {
    let parsed = parse_java_source(
        r#"
import java.util.ArrayList;

class LocalGenericArrayArgumentSyntax {
    void read() {
        ArrayList<String[]> group = new ArrayList<>();
    }
}
"#,
    );
    let body = method_body(&parsed.program.types[0], "read");
    let Stmt::VarDecl { ty, .. } = &body.statements[0] else {
        panic!("expected local declaration");
    };
    assert_eq!(
        ty,
        &JavaType::Named {
            name: "ArrayList".to_string(),
            args: vec![JavaType::Array(Box::new(JavaType::String))],
        }
    );
}

#[test]
fn parses_parameter_array_dimensions_after_name() {
    let parsed = parse_java_source(
        r#"
class ParameterArrayDimensionSyntax {
    void split(final double d, final double result[]) {
    }
}
"#,
    );
    let Member::Method(method) = &parsed.program.types[0].members[0] else {
        panic!("expected method");
    };
    assert_eq!(
        method.params[0].ty,
        JavaType::Primitive(PrimitiveType::Double)
    );
    assert_eq!(
        method.params[1].ty,
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Double)))
    );
}

#[test]
fn parses_enhanced_for_array_variable_dimensions() {
    let parsed = parse_java_source(
        r#"
class EnhancedForArrayVariableDimensions {
    void walk(String[][] group) {
        for (String[] seg : group) {
            seg[0].trim();
        }
    }
}
"#,
    );
    let body = method_body(&parsed.program.types[0], "walk");

    let Stmt::EnhancedFor { var_type, .. } = &body.statements[0] else {
        panic!("expected enhanced for statement");
    };
    assert!(matches!(
        var_type,
        JavaType::Array(inner) if matches!(inner.as_ref(), JavaType::String)
    ));
}

#[test]
fn parses_field_array_dimensions_after_name() {
    let parsed = parse_java_source(
        r#"
class FieldArrayDimensionSyntax {
    private static final double FACT[] = new double[] { 1.0, 2.0 };
    private static final double COEF[][] = { { 1.0, 2.0 } };
}
"#,
    );
    let fields = parsed.program.types[0]
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fields[0].ty,
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Double)))
    );
    assert_eq!(
        fields[1].ty,
        JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double
        )))))
    );
}

#[test]
fn parses_typed_array_initializer_without_raw_fallbacks() {
    let expr = new_java2rust::parser::parse_expr("new String[]{prefix + comment}");
    let Expr::ArrayCreation {
        element_type,
        dimensions,
        initializer: Some(elements),
    } = expr
    else {
        panic!("expected typed array initializer");
    };

    assert_eq!(element_type, JavaType::String);
    assert!(dimensions.is_empty());
    assert_eq!(elements.len(), 1);
}

#[test]
fn parses_array_initializer_ignores_comment_nodes() {
    let expr = new_java2rust::parser::parse_expr(
        r#"new double[] {
            +1.0d, // first
            +2.0d  // second
        }"#,
    );
    let Expr::ArrayCreation {
        initializer: Some(elements),
        ..
    } = expr
    else {
        panic!("expected typed array initializer");
    };
    assert_eq!(elements.len(), 2);
    assert!(matches!(
        &elements[0],
        Expr::Unary {
            op: UnaryOp::Pos,
            expr
        } if matches!(expr.as_ref(), Expr::Literal(Literal::Double(1.0)))
    ));
    assert!(matches!(
        &elements[1],
        Expr::Unary {
            op: UnaryOp::Pos,
            expr
        } if matches!(expr.as_ref(), Expr::Literal(Literal::Double(2.0)))
    ));
}

#[test]
fn parses_hex_long_sign_bit_as_twos_complement_literal() {
    let expr = new_java2rust::parser::parse_expr("0x8000000000000000L");

    assert!(matches!(expr, Expr::Literal(Literal::Long(i64::MIN))));
}

#[test]
fn parses_hex_int_sign_bit_as_twos_complement_literal() {
    let expr = new_java2rust::parser::parse_expr("0x80000000");

    assert!(matches!(expr, Expr::Literal(Literal::Int(i32::MIN))));
}

#[test]
fn parses_java_hex_floating_point_literals() {
    let double_expr = new_java2rust::parser::parse_expr("0x1.0p-52d");
    let float_expr = new_java2rust::parser::parse_expr("0x1.0p-23f");

    assert!(matches!(
        double_expr,
        Expr::Literal(Literal::Double(value)) if value == 2f64.powi(-52)
    ));
    assert!(matches!(
        float_expr,
        Expr::Literal(Literal::Float(value)) if value == 2f32.powi(-23)
    ));
}

#[test]
fn parses_parenthesized_condition_after_block_comment() {
    let parsed = parse_java_source(
        r#"
class CommentedCondition {
    boolean done(int position, int numSigBytes) {
        if (/*!encode &&*/ position >= numSigBytes) {
            return true;
        }
        return false;
    }
}
"#,
    );
    assert!(collect_program_fallbacks(&parsed.program).is_empty());

    let body = method_body(&parsed.program.types[0], "done");
    let Stmt::If { condition, .. } = &body.statements[0] else {
        panic!("expected if statement");
    };
    assert!(matches!(
        condition,
        Expr::Binary {
            op: BinaryOp::Ge,
            left,
            right,
        } if matches!(left.as_ref(), Expr::Var(name) if name == "position")
            && matches!(right.as_ref(), Expr::Var(name) if name == "num_sig_bytes")
    ));
}

#[test]
fn parses_gatk_top_level_enum_without_no_class_error() {
    let parsed = parse_java_source(&parser_gap("GatkTopLevelEnum.java"));

    assert!(!parsed.diagnostics.has_code("parse_no_class"));
    assert_eq!(parsed.program.types.len(), 1);
    assert_eq!(parsed.program.types[0].kind, TypeKind::Enum);
    assert_eq!(parsed.program.types[0].name, "GatkTopLevelEnum");
    assert!(collect_program_fallbacks(&parsed.program).is_empty());
}

#[test]
fn parses_enum_constants_preserving_java_constant_labels() {
    let parsed = parse_java_source(
        r#"
enum LocalizedFormats {
    MAX_COUNT_EXCEEDED("max"),
    ILLEGAL_STATE("bad");

    LocalizedFormats(String source) {}
}
"#,
    );

    assert!(!parsed.diagnostics.has_code("parse_no_class"));
    let ty = &parsed.program.types[0];
    let enum_fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field.name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(enum_fields, vec!["MAX_COUNT_EXCEEDED", "ILLEGAL_STATE"]);
}

#[test]
fn parses_gatk_top_level_annotation_without_no_class_error() {
    let parsed = parse_java_source(&parser_gap("GatkTopLevelAnnotation.java"));

    assert!(!parsed.diagnostics.has_code("parse_no_class"));
    assert_eq!(parsed.program.types.len(), 1);
    assert_eq!(parsed.program.types[0].kind, TypeKind::Interface);
    assert_eq!(parsed.program.types[0].name, "GatkTopLevelAnnotation");
}

#[test]
fn parses_gatk_package_info_without_no_class_error() {
    let parsed = parse_java_source(&parser_gap("GatkPackageInfo.java"));

    assert!(parsed.program.types.is_empty());
    assert_eq!(parsed.program.package.as_deref(), Some("parser.gaps"));
    assert!(!parsed.diagnostics.has_code("parse_no_class"));
}

#[test]
fn parses_gatk_line_comment_in_switch_without_raw_fallback() {
    let parsed = parse_java_source(&parser_gap("GatkLineCommentStatement.java"));
    let fallbacks = collect_program_fallbacks(&parsed.program);

    assert!(fallbacks.is_empty(), "{fallbacks:?}");
}

#[test]
fn parses_gatk_empty_statement_without_raw_fallback() {
    let parsed = parse_java_source(&parser_gap("GatkEmptyStatement.java"));
    let fallbacks = collect_program_fallbacks(&parsed.program);

    assert!(fallbacks.is_empty(), "{fallbacks:?}");
}

#[test]
fn static_initializer_block_surfaces_as_unsupported_member() {
    let parsed = parse_java_source(
        r#"
class StaticInitializerUnsupportedMember {
    static int value = 0;

    static {
        value = 3;
    }

    int read() {
        return value;
    }
}
"#,
    );
    let fallbacks = collect_program_fallbacks(&parsed.program);

    assert!(
        fallbacks
            .iter()
            .any(|fallback| fallback.contains("static_initializer")),
        "{fallbacks:?}"
    );
}

fn method_body<'a>(
    ty: &'a new_java2rust::java_ir::TypeDecl,
    rust_name: &str,
) -> &'a new_java2rust::java_ir::Block {
    ty.members
        .iter()
        .find_map(|member| match member {
            Member::Method(method)
                if method.rust_name.as_deref() == Some(rust_name) || method.name == rust_name =>
            {
                method.body.as_ref()
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing method {rust_name}"))
}

fn collect_program_fallbacks(program: &new_java2rust::java_ir::Program) -> Vec<String> {
    let mut fallbacks = Vec::new();
    for ty in &program.types {
        for member in &ty.members {
            collect_member_fallbacks(member, &mut fallbacks);
        }
    }
    fallbacks
}

fn collect_member_fallbacks(member: &Member, out: &mut Vec<String>) {
    match member {
        Member::Field(field) => {
            if let Some(expr) = &field.initializer {
                collect_expr_fallbacks(expr, out);
            }
        }
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_block_fallbacks(body, out);
            }
        }
        Member::Constructor(ctor) => {
            if let Some(body) = &ctor.body {
                collect_block_fallbacks(body, out);
            }
        }
        Member::Type(ty) => {
            for member in &ty.members {
                collect_member_fallbacks(member, out);
            }
        }
        Member::Unsupported { reason, .. } => out.push(format!("member unsupported: {reason}")),
    }
}

fn collect_block_fallbacks(block: &new_java2rust::java_ir::Block, out: &mut Vec<String>) {
    for stmt in &block.statements {
        collect_stmt_fallbacks(stmt, out);
    }
}

fn collect_stmt_fallbacks(stmt: &Stmt, out: &mut Vec<String>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                collect_expr_fallbacks(expr, out);
            }
        }
        Stmt::Assign { target, value } => {
            collect_expr_fallbacks(target, out);
            collect_expr_fallbacks(value, out);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_fallbacks(expr, out);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_fallbacks(condition, out);
            collect_block_fallbacks(then_block, out);
            if let Some(block) = else_block {
                collect_block_fallbacks(block, out);
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_fallbacks(condition, out);
            collect_block_fallbacks(body, out);
        }
        Stmt::DoWhile { body, condition } => {
            collect_block_fallbacks(body, out);
            collect_expr_fallbacks(condition, out);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_fallbacks(stmt, out);
            }
            if let Some(condition) = condition {
                collect_expr_fallbacks(condition, out);
            }
            for expr in update {
                collect_expr_fallbacks(expr, out);
            }
            collect_block_fallbacks(body, out);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_fallbacks(iterable, out);
            collect_block_fallbacks(body, out);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_fallbacks(selector, out);
            for case in cases {
                for label in &case.labels {
                    if let new_java2rust::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_expr_fallbacks(expr, out);
                    }
                }
                for stmt in &case.statements {
                    collect_stmt_fallbacks(stmt, out);
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
                collect_stmt_fallbacks(resource, out);
            }
            collect_block_fallbacks(body, out);
            for catch in catches {
                collect_block_fallbacks(&catch.body, out);
            }
            if let Some(block) = finally_block {
                collect_block_fallbacks(block, out);
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                collect_member_fallbacks(member, out);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
        }
        Stmt::Block(block) => collect_block_fallbacks(block, out),
        Stmt::Unsupported(reason) => out.push(format!("stmt unsupported: {reason}")),
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expr_fallbacks(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Raw(raw) => out.push(format!("expr raw: {raw}")),
        Expr::FieldAccess(receiver, _) => collect_expr_fallbacks(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_expr_fallbacks(array, out);
            collect_expr_fallbacks(index, out);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_expr_fallbacks(expr, out);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_expr_fallbacks(expr, out);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_expr_fallbacks(expr, out);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_fallbacks(left, out);
            collect_expr_fallbacks(right, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_fallbacks(expr, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_fallbacks(condition, out);
            collect_expr_fallbacks(then_expr, out);
            collect_expr_fallbacks(else_expr, out);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_fallbacks(receiver, out);
            }
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
            if let Some(members) = body {
                for member in members {
                    collect_member_fallbacks(member, out);
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            new_java2rust::java_ir::LambdaBody::Expr(expr) => collect_expr_fallbacks(expr, out),
            new_java2rust::java_ir::LambdaBody::Block(block) => collect_block_fallbacks(block, out),
        },
        _ => {}
    }
}
