#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_class_registries_merge_across_independent_source_modules() {
    let sources = [
        (
            "producer/Units.java",
            include_str!("corpus/numbers/angle-lowering/producer/Units.java"),
        ),
        (
            "consumer/ClassTokenBatchProbe.java",
            include_str!("corpus/numbers/angle-lowering/consumer/ClassTokenBatchProbe.java"),
        ),
    ];
    let paths = sources
        .iter()
        .map(|(path, source)| {
            (
                format!("tests/corpus/numbers/angle-lowering/{path}"),
                *source,
            )
        })
        .collect::<Vec<_>>();
    let oracle = paths
        .iter()
        .map(|(path, source)| (path.as_str(), *source))
        .collect::<Vec<_>>();
    common::verify_java_sources_zero_contract("consumer.ClassTokenBatchProbe", &oracle);
    let batch = sources
        .iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(*path, *source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "erased_class_batch",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ClassTokenBatchProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn erased_class_dispatch_uses_actual_payload_and_hierarchy_metadata() {
    common::verify_java_fixture_zero_contract(
        "erased_class_dispatch",
        "erasedclasses.ErasedClassProbe",
        "tests/corpus/numbers/ErasedClassProbe.java",
        include_str!("corpus/numbers/ErasedClassProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn actual_classes() { assert_eq!(ErasedClassProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn scalar_min_max_preserve_nan_operands_and_existing_zero_ordering() {
    common::verify_java_fixture_zero_contract(
        "math_operand_retention",
        "mathoperand.MathOperandProbe",
        "tests/corpus/numbers/MathOperandProbe.java",
        include_str!("corpus/numbers/MathOperandProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn operands_and_zero_order() { assert_eq!(MathOperandProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn conditional_effects_distinguish_condition_from_joined_branch_carriers() {
    common::verify_java_fixture_zero_contract(
        "conditional_effects",
        "conditioneffects.ConditionalEffectsProbe",
        "tests/corpus/numbers/ConditionalEffectsProbe.java",
        include_str!("corpus/numbers/ConditionalEffectsProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn conditional_values_and_errors() { assert_eq!(ConditionalEffectsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nested_source_static_fields_use_storage_accessors_across_summary_copies() {
    let sources = [
        (
            "producer/Units.java",
            include_str!("corpus/numbers/angle-lowering/producer/Units.java"),
        ),
        (
            "consumer/NestedStaticProbe.java",
            include_str!("corpus/numbers/angle-lowering/consumer/NestedStaticProbe.java"),
        ),
    ];
    let paths = sources
        .iter()
        .map(|(path, source)| {
            (
                format!("tests/corpus/numbers/angle-lowering/{path}"),
                *source,
            )
        })
        .collect::<Vec<_>>();
    let oracle = paths
        .iter()
        .map(|(path, source)| (path.as_str(), *source))
        .collect::<Vec<_>>();
    common::verify_java_sources_zero_contract("consumer.NestedStaticProbe", &oracle);
    let batch = sources
        .iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(*path, *source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "nested_static_source",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::NestedStaticProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nested_erased_arguments_propagate_effects_and_preserve_evaluation_order() {
    common::verify_java_fixture_zero_contract(
        "nested_argument_effects",
        "nestedeffects.NestedArgumentEffectsProbe",
        "tests/corpus/numbers/NestedArgumentEffectsProbe.java",
        include_str!("corpus/numbers/NestedArgumentEffectsProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn errors_and_order() { assert_eq!(NestedArgumentEffectsProbe::run().value(), 0); }
"#,
    );
}
