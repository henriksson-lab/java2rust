#[path = "common/mod.rs"]
mod common;

#[test]
fn bulk_collection_calls_preserve_java_operand_order_and_borrow_sources() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/bulk_collection_operands.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "bulk_collection_operands",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn operands_are_eager_ordered_and_evaluated_once() {
    for (operation, expected) in [
        (BulkCollectionOperands::contains as fn(i32, i32) -> Result<bool, JavaException>, false),
        (BulkCollectionOperands::adds, true),
        (BulkCollectionOperands::removes, true),
        (BulkCollectionOperands::retains, true),
    ] {
        BulkCollectionOperands::reset();
        assert_eq!(operation(0, 0).unwrap(), expected);
        assert_eq!(BulkCollectionOperands::trace(), 12);
        for (first, second, kind, trace) in [
            (1, 0, "IllegalArgumentException", 1),
            (2, 0, "NullPointerException", 12),
            (2, 3, "NullPointerException", 12),
            (2, 1, "IllegalStateException", 12),
            (0, 2, "NullPointerException", 12),
        ] {
            BulkCollectionOperands::reset();
            let error = operation(first, second).unwrap_err();
            assert!(error.is_instance(&[kind]), "{error:?}");
            assert_eq!(BulkCollectionOperands::trace(), trace);
        }
    }
    BulkCollectionOperands::reset();
    assert!(BulkCollectionOperands::contains(0, 3).unwrap());
    assert_eq!(BulkCollectionOperands::trace(), 12);
    assert!(BulkCollectionOperands::name_collision(&[1, 2], &[2]));
    assert!(!BulkCollectionOperands::name_collision(&[1], &[2]));
    assert!(BulkCollectionOperands::staging_names(&[1, 2], &[2]));
    assert_eq!(BulkCollectionOperands::cross_kinds(), 321);
    assert_eq!(BulkCollectionOperands::new().field_self(0).unwrap(), 40);
    let error = BulkCollectionOperands::new().field_self(1).unwrap_err();
    assert!(error.is_instance(&["NullPointerException"]));
    for (operation, expected) in [
        (BulkCollectionOperands::same_call_contains as fn() -> Result<bool, JavaException>, true),
        (BulkCollectionOperands::same_call_removes, true),
        (BulkCollectionOperands::same_call_retains, false),
    ] {
        BulkCollectionOperands::reset();
        assert_eq!(operation().unwrap(), expected);
        assert_eq!(BulkCollectionOperands::trace(), 11);
    }
    let mut values = vec![1, 2];
    assert!(BulkCollectionOperands::self_contains(&values));
    assert!(!BulkCollectionOperands::self_retains(&mut values));
    assert_eq!(values, vec![1, 2]);
    assert!(BulkCollectionOperands::self_adds(&mut values));
    assert_eq!(values, vec![1, 2, 1, 2]);
    assert!(BulkCollectionOperands::self_removes(&mut values));
    assert!(values.is_empty());
    assert!(!BulkCollectionOperands::self_removes(&mut values));
    for operation in [BulkCollectionOperands::add_statement, BulkCollectionOperands::retain_statement] {
        BulkCollectionOperands::reset();
        operation(0, 0).unwrap();
        assert_eq!(BulkCollectionOperands::trace(), 12);
        BulkCollectionOperands::reset();
        let error = operation(2, 1).unwrap_err();
        assert!(error.is_instance(&["IllegalStateException"]));
        assert_eq!(BulkCollectionOperands::trace(), 12);
    }
}
"#,
        }],
    );
}
