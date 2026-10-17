#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_null_arguments_retain_the_resolved_caller_frame() {
    use new_java2rust::java_ir::{Expr, JavaType, Member, Stmt};
    let parsed = new_java2rust::parser::parse_java_source(include_str!(
        "fixtures/generic_value_equality.java"
    ));
    let resolved = new_java2rust::resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ValueEquality")
        .unwrap();
    for (name, null_index) in [("forceNullableRight", 1), ("nullable", 0)] {
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        let Stmt::Return(Some(Expr::Call {
            resolved: Some(call),
            ..
        })) = &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("call");
        };
        assert_eq!(
            call.arg_target_types[null_index],
            JavaType::String,
            "{name}: {call:?}"
        );
    }
}

#[test]
fn generic_equals_uses_java_equality_without_rust_structural_traits() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/generic_value_equality.java"));
    assert!(
        !translation
            .rust
            .contains("struct __JavaGenericRetargetEqualityBound"),
        "a borrowed dependent argument has no owned retarget proof"
    );
    common::cargo_test_generated_modules(
        "generic_value_equality",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NeverClone;
struct JavaOnly(i32);
impl JavaMapKey for JavaOnly {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 % 10 == other.0 % 10 }
}
struct BoundOnly;
impl EqualityBound<BoundOnly> for BoundOnly {
    fn equals(&self, _other: &dyn EqualityBound<BoundOnly>) -> bool { false }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn EqualityBound<BoundOnly>> { Box::new(BoundOnly) }
}
#[derive(Clone)]
struct Source(std::rc::Rc<std::cell::Cell<i32>>);
impl EqualitySource<JavaOnly> for Source {
    fn get(&self) -> JavaOnly { self.0.set(self.0.get() + 1); JavaOnly(17) }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn EqualitySource<JavaOnly>> { Box::new(self.clone()) }
}
#[test]
fn java_equals_is_not_partial_eq_or_reference_comparison() {
    assert!(ValueEquality::same(&JavaOnly(7), &JavaOnly(17)));
    assert!(!ValueEquality::different(&JavaOnly(7), &JavaOnly(17)));
    assert!(ValueEquality::different(&JavaOnly(7), &JavaOnly(18)));
    assert!(ValueEquality::via(&JavaOnly(7), &JavaOnly(17)));
    assert!(!ValueEquality::bounded(&BoundOnly, &BoundOnly));
    assert!(!ValueEquality::nullable_right(&JavaOnly(7), None));
    assert!(ValueEquality::nullable_right(&JavaOnly(7), Some(&JavaOnly(17))));
    let view: Box<dyn EqualityDefault<NeverClone, JavaOnly>> =
        Box::new(EqualityImplementation::<NeverClone, JavaOnly>::new());
    assert!(view.same(JavaOnly(7), JavaOnly(17)));
    assert!(view.different(JavaOnly(7), JavaOnly(18)));
    assert!(!OwnEquality::run());
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let source = Source(calls.clone());
    assert!(ValueEquality::from::<JavaOnly>(None, &source).is_err());
    assert_eq!(calls.get(), 1, "argument evaluated before null receiver error");
    assert!(ValueEquality::from(Some(&JavaOnly(7)), &source).unwrap());
    assert_eq!(calls.get(), 2);
    assert!(ValueEquality::from_not::<JavaOnly>(None, &source).is_err());
    assert_eq!(calls.get(), 3);
    assert!(!ValueEquality::from_not(Some(&JavaOnly(7)), &source).unwrap());
    assert_eq!(calls.get(), 4);
}
"#,
        }],
    );
}

#[test]
fn generic_reference_operator_does_not_acquire_value_equality_capabilities() {
    let translation = new_java2rust::translate_source(
        "class Identity { static <T> boolean same(T left, T right) { return left == right; } }",
    );
    let method = translation.rust.split("pub fn same<T>").nth(1).unwrap();
    let body = method.split("\n    }").next().unwrap();
    assert!(body.contains("=="), "{body}");
    assert!(!body.contains("java_value_equals"), "{body}");
    assert!(!body.contains("java_nullable_value_equals"), "{body}");
}
