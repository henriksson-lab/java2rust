use new_java2rust::coercion::{Coercion, Slot, plan_coercion};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::parser::{parse_expr, parse_type};
use new_java2rust::types::{TypeEnv, infer_expr_type, is_string_concat};

#[test]
fn type_inference_owns_binary_numeric_promotion() {
    let mut env = TypeEnv::new();
    env.insert_local("b", JavaType::Primitive(PrimitiveType::Byte));
    env.insert_local("s", JavaType::Primitive(PrimitiveType::Short));
    env.insert_local("i", JavaType::Primitive(PrimitiveType::Int));

    let ty = infer_expr_type(&parse_expr("b + s + i"), &env);
    assert_eq!(ty, JavaType::Primitive(PrimitiveType::Int));
}

#[test]
fn type_inference_owns_java_length_type() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
    );

    let ty = infer_expr_type(&parse_expr("values.length"), &env);
    assert_eq!(ty, JavaType::Primitive(PrimitiveType::Int));
}

#[test]
fn type_inference_identifies_string_concat() {
    let mut env = TypeEnv::new();
    env.insert_local("name", JavaType::String);
    env.insert_local("count", JavaType::Primitive(PrimitiveType::Int));

    assert!(is_string_concat(
        &parse_expr("\"name=\" + name + count"),
        &env
    ));
}

#[test]
fn coercion_planner_owns_index_and_widening_slots() {
    assert_eq!(
        plan_coercion(&JavaType::Primitive(PrimitiveType::Int), &Slot::Index),
        vec![Coercion::IndexToUsize]
    );

    assert_eq!(
        plan_coercion(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Return(JavaType::Primitive(PrimitiveType::Long))
        ),
        vec![Coercion::WidenPrimitive {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long
        }]
    );
}

#[test]
fn parser_keeps_generic_type_shape_for_type_pass() {
    assert_eq!(
        parse_type("List<Integer>"),
        JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new()
            }]
        }
    );
}
