#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_receiver_arguments_release_borrows_and_preserve_null() {
    common::verify_java_fixture_zero_contract(
        "nullable_receiver_arguments",
        "NullableReceiverArgumentProbe",
        "tests/corpus/numbers/NullableReceiverArgumentProbe.java",
        include_str!("corpus/numbers/NullableReceiverArgumentProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn nullable() { assert_eq!(NullableReceiverArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_source_results_are_erased_elementwise_at_object_arguments() {
    common::verify_java_fixture_zero_contract(
        "nullable_source_equality",
        "NullableSourceEqualityProbe",
        "tests/corpus/numbers/NullableSourceEqualityProbe.java",
        include_str!("corpus/numbers/NullableSourceEqualityProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn nullable() { assert_eq!(NullableSourceEqualityProbe::run().value(), 0); }
"#,
    );
}

macro_rules! check {
    ($body:literal) => {
        concat!(
            "trait Value { fn value(self) -> i32; }\n",
            "impl Value for i32 { fn value(self) -> i32 { self } }\n",
            "impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }\n",
            $body,
        )
    };
}

#[test]
fn boxed_primitive_equals_uses_existing_erased_value_contracts() {
    common::verify_java_fixture_zero_contract(
        "boxed_primitive_equals",
        "BoxedPrimitiveEqualsProbe",
        "tests/corpus/numbers/BoxedPrimitiveEqualsProbe.java",
        include_str!("corpus/numbers/BoxedPrimitiveEqualsProbe.java"),
        check!("#[test] fn values() { assert_eq!(BoxedPrimitiveEqualsProbe::run().value(), 0); }"),
    );
}

#[test]
fn retained_array_equality_compares_reference_snapshots() {
    common::verify_java_fixture_zero_contract(
        "array_reference_identity",
        "ArrayReferenceIdentityProbe",
        "tests/corpus/numbers/ArrayReferenceIdentityProbe.java",
        include_str!("corpus/numbers/ArrayReferenceIdentityProbe.java"),
        check!(
            "#[test] fn identity() { assert_eq!(ArrayReferenceIdentityProbe::run().value(), 0); }"
        ),
    );
}

#[test]
fn source_boxed_lookalike_keeps_source_dispatch() {
    common::verify_java_fixture_zero_contract(
        "source_boxed_lookalike",
        "SourceBoxedLookalikeProbe",
        "tests/corpus/numbers/SourceBoxedLookalikeProbe.java",
        include_str!("corpus/numbers/SourceBoxedLookalikeProbe.java"),
        check!("#[test] fn source() { assert_eq!(SourceBoxedLookalikeProbe::run().value(), 0); }"),
    );
}
