use new_java2rust::coercion::{
    Coercion, DispatchShape, LoweredExpr, LoweredType, OwnershipShape, Slot, ViewShape,
    apply_expr_to_slot, apply_instanceof_check, apply_lowered_expr_to_slot, plan_coercion,
    plan_shape_coercion,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::parser::{parse_expr, parse_type};
use new_java2rust::rust_ir::RustExpr;
use new_java2rust::slot_shape::{RustSlotShape, list_java_type, optional_java_type};
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
fn coercion_planner_owns_index_and_shape_widening() {
    assert_eq!(
        plan_coercion(&JavaType::Primitive(PrimitiveType::Int), &Slot::Index),
        vec![Coercion::IndexToUsize]
    );

    let actual = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::WidenPrimitive {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long
        }]
    );
}

#[test]
fn shape_coercion_owns_string_view_adapters() {
    let actual = RustSlotShape::conservative(JavaType::String);
    let mut target = RustSlotShape::conservative(JavaType::String);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Str;

    assert_eq!(plan_shape_coercion(&actual, &target), vec![Coercion::AsStr]);

    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("name".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw("name.as_str()".to_string())
    );
}

#[test]
fn shape_coercion_owns_closed_enum_upcast_and_downcast_adapters() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("dog".to_string()),
            actual,
            &Slot::Assignment(target.clone())
        ),
        RustExpr::Raw("AnimalValue::Dog(dog)".to_string())
    );

    let mut enum_actual = target;
    enum_actual.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let downcast_target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });

    assert_eq!(
        plan_shape_coercion(&enum_actual, &downcast_target),
        vec![Coercion::DowncastClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }]
    );
}

#[test]
fn coercion_owns_closed_enum_instanceof_predicates() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });

    assert_eq!(
        apply_instanceof_check(RustExpr::Var("animal".to_string()), &actual, &target),
        RustExpr::Raw("matches!(animal, AnimalValue::Dog(_))".to_string())
    );
}

#[test]
fn shape_coercion_owns_dyn_trait_and_erased_object_adapters() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&actual, &dyn_target),
        vec![Coercion::BorrowDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("dog".to_string()),
            actual.clone(),
            &Slot::Argument(dyn_target)
        ),
        RustExpr::Raw("&dog as &dyn Animal".to_string())
    );

    let mut boxed_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion(&actual, &boxed_target),
        vec![Coercion::BoxDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );

    let mut erased_target = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_target.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        plan_shape_coercion(&actual, &erased_target),
        vec![Coercion::EraseObject]
    );
}

#[test]
fn coercion_owns_borrowed_optional_to_owned_optional_slot() {
    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(JavaType::String),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::String,
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("values.iter().next()".to_string())),
            method: "cloned".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn coercion_uses_copy_capability_for_borrowed_optional_elements() {
    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(JavaType::Primitive(PrimitiveType::Int)),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::Primitive(PrimitiveType::Int),
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("values.iter().next()".to_string())),
            method: "copied".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn coercion_uses_element_shape_for_borrowed_vec_string_view() {
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;
    element_shape.view = ViewShape::Str;

    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().map(|v| v.trim()).collect::<Vec<_>>()".to_string()),
            actual: LoweredType::vec_element_borrow(element_shape),
        },
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::Raw(
            "values.iter().map(|v| v.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
                .to_string()
        )
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
