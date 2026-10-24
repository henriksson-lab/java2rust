#[path = "common/mod.rs"]
mod common;

#[test]
fn initialized_enum_receivers_accept_fresh_arrays_in_nullable_varargs_slots() {
    common::verify_java_fixture_zero_contract(
        "nullable_array_creation",
        "NullableArrayCreationProbe",
        "tests/corpus/numbers/NullableArrayCreationProbe.java",
        include_str!("corpus/numbers/NullableArrayCreationProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn boundary() { assert_eq!(NullableArrayCreationProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn object_varargs_distinguish_explicit_arrays_and_array_valued_elements() {
    common::verify_java_fixture_zero_contract(
        "object_varargs_shape",
        "ObjectVarargsShapeProbe",
        "tests/corpus/numbers/ObjectVarargsShapeProbe.java",
        include_str!("corpus/numbers/ObjectVarargsShapeProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arity() { assert_eq!(ObjectVarargsShapeProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn reference_rows_preserve_repeated_aliases_replacement_and_validation_order() {
    common::verify_java_fixture_zero_contract(
        "reference_rows",
        "ReferenceRowsProbe",
        "tests/corpus/numbers/ReferenceRowsProbe.java",
        include_str!("corpus/numbers/ReferenceRowsProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn rows() { assert_eq!(ReferenceRowsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn stored_comparators_order_non_comparable_objects_and_keep_natural_failures_lazy() {
    common::verify_java_fixture_zero_contract(
        "stored_comparator",
        "StoredComparatorProbe",
        "tests/corpus/numbers/StoredComparatorProbe.java",
        include_str!("corpus/numbers/StoredComparatorProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn ordering() { assert_eq!(StoredComparatorProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn array_boundary_projections_preserve_aliases_nulls_and_terminal_loops() {
    let source = include_str!("corpus/numbers/ArrayBoundaryProjectionProbe.java");
    common::verify_java_sources_zero_contract(
        "ArrayBoundaryProjectionProbe",
        &[(
            "tests/corpus/numbers/ArrayBoundaryProjectionProbe.java",
            source,
        )],
    );
    for identity in [false, true] {
        let mut facts = new_java2rust::facts::FactSet::default();
        facts.java_string_identity_representation = identity;
        let translated = new_java2rust::translate_source_with_facts(source, &facts);
        assert!(
            !translated.diagnostics.has_errors(),
            "{:?}",
            translated.diagnostics
        );
        common::cargo_verify_generated_modules_with_edition(
            if identity {
                "array_boundary_identity"
            } else {
                "array_boundary_value"
            },
            vec![common::GeneratedCrateModule {
                name: "fixture",
                rust: translated.rust,
                tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn boundary() { assert_eq!(ArrayBoundaryProjectionProbe::run().value(), 0); }
"#,
            }],
            "test",
            "2021",
        );
    }
}
