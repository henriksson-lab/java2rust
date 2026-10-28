#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_arguments_keep_physical_storage_through_generic_and_interface_dispatch() {
    common::verify_java_fixture_zero_contract(
        "retained_dispatch_argument",
        "RetainedDispatchArgumentProbe",
        "tests/fixtures/RetainedDispatchArgumentProbe.java",
        include_str!("fixtures/RetainedDispatchArgumentProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(RetainedDispatchArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn covariant_reader_products_keep_reader_identity_dispatch_and_concrete_product_types() {
    let source = include_str!("fixtures/CovariantReaderProductProbe.java");
    let interface = include_str!("fixtures/ReaderProduct.java");
    common::verify_java_sources_zero_contract(
        "CovariantReaderProductProbe",
        &[
            ("tests/fixtures/CovariantReaderProductProbe.java", source),
            ("tests/fixtures/ReaderProduct.java", interface),
        ],
    );
    // Match Jahmm's public generic product API; verify both source files with
    // javac, then exercise their complete declarations in one lowering unit.
    let translated = new_java2rust::translate_source(&format!("{interface}\n{source}"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "covariant_reader_product",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn readers() { assert_eq!(CovariantReaderProductProbe::run().value(), 0); }
struct ProductAllocationCounter;
std::thread_local! {
    static PRODUCT_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COUNT_PRODUCT_ALLOCATIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
unsafe impl std::alloc::GlobalAlloc for ProductAllocationCounter {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let pointer = unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) };
        if !pointer.is_null() && COUNT_PRODUCT_ALLOCATIONS.try_with(|active| active.get()).unwrap_or(false) {
            let _ = PRODUCT_ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout); }
    }
}
#[global_allocator] static PRODUCT_ALLOCATOR: ProductAllocationCounter = ProductAllocationCounter;
type FirstProductHandle = std::rc::Rc<std::cell::RefCell<FirstReaderProduct>>;
trait ProductSuccess { fn product(self) -> FirstProductHandle; }
impl ProductSuccess for FirstProductHandle { fn product(self) -> FirstProductHandle { self } }
impl<E: std::fmt::Debug> ProductSuccess for Result<FirstProductHandle,E> { fn product(self) -> FirstProductHandle { self.unwrap() } }
#[test] fn exact_reader_dispatch_has_no_transient_erased_box() {
    struct NotClone;
    let _clone_without_product_bound: fn(&ProductReaderValue<NotClone>) -> ProductReaderValue<NotClone> = Clone::clone;
    let mut reader: ProductReaderValue<FirstProductHandle> = FirstProductReader::new().into();
    drop(reader.read(0).product());
    PRODUCT_ALLOCATIONS.with(|count| count.set(0));
    COUNT_PRODUCT_ALLOCATIONS.with(|active| active.set(true));
    let product = reader.read(17).product();
    COUNT_PRODUCT_ALLOCATIONS.with(|active| active.set(false));
    // Only the freshly created canonical product needs an allocation.
    assert_eq!(PRODUCT_ALLOCATIONS.with(|count| count.get()), 1);
    assert_eq!(product.borrow().value, 17);
}
"#,
        }],
    );
}

#[test]
fn source_varargs_keep_array_packing_after_receiver_field_argument_staging() {
    common::verify_java_fixture_zero_contract(
        "staged_varargs",
        "StagedVarargsProbe",
        "tests/fixtures/StagedVarargsProbe.java",
        include_str!("fixtures/StagedVarargsProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn varargs() { assert_eq!(StagedVarargsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nullable_cursor_results_cross_ordinary_and_inherited_constructor_locals() {
    common::verify_java_fixture_zero_contract(
        "nullable_cursor_domain",
        "NullableCursorDomainProbe",
        "tests/fixtures/NullableCursorDomainProbe.java",
        include_str!("fixtures/NullableCursorDomainProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn cursors() { assert_eq!(NullableCursorDomainProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn nested_covariant_sequences_preserve_child_carriers_across_signatures_storage_and_loops() {
    common::verify_java_fixture_zero_contract(
        "nested_sequence_carrier",
        "NestedSequenceCarrierProbe",
        "tests/fixtures/NestedSequenceCarrierProbe.java",
        include_str!("fixtures/NestedSequenceCarrierProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn sequences() { assert_eq!(NestedSequenceCarrierProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn fresh_collection_arrays_preserve_dimension_effects_nulls_aliases_and_requested_length() {
    common::verify_java_fixture_zero_contract(
        "fresh_collection_array",
        "FreshCollectionArrayProbe",
        "tests/fixtures/FreshCollectionArrayProbe.java",
        include_str!("fixtures/FreshCollectionArrayProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(FreshCollectionArrayProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn unsupported_collection_destinations_are_explicit_and_source_methods_keep_their_contract() {
    let unsupported = new_java2rust::translate_source(
        r#"
        import java.util.List;
        class IntoArray {
            static String[] into(List<Object> values, String[] target) { return values.toArray(target); }
        }
    "#,
    );
    assert!(unsupported.rust.contains(
        "typed Collection.toArray requires a compatible fresh reference-array destination or a retained destination with a proved exact final component"
    ), "{}", unsupported.rust);
    let source = new_java2rust::translate_source(
        r#"
        class ArrayApi {
            String[] toArray(String[] target) { return target; }
            static String[] run() { return new ArrayApi().toArray(new String[2]); }
        }
    "#,
    );
    assert!(!source.diagnostics.has_errors(), "{:?}", source.diagnostics);
    assert!(!source.rust.contains("typed Collection.toArray requires"));
}

#[test]
fn partial_array_locals_preserve_solved_carriers_through_rows_and_constructor_storage() {
    common::verify_java_fixture_zero_contract(
        "partial_array_carrier",
        "PartialArrayCarrierProbe",
        "tests/fixtures/PartialArrayCarrierProbe.java",
        include_str!("fixtures/PartialArrayCarrierProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(PartialArrayCarrierProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn constructor_validation_reads_staged_fields_without_fabricating_default_objects() {
    common::verify_java_fixture_zero_contract(
        "constructor_prefix",
        "ConstructorPrefixProbe",
        "tests/fixtures/ConstructorPrefixProbe.java",
        include_str!("fixtures/ConstructorPrefixProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn constructors() { assert_eq!(ConstructorPrefixProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn inherited_generic_source_results_use_the_emitted_callee_contract_across_units() {
    common::cargo_run_translated_batch_sources(
        "inherited_size_contract",
        &[
            new_java2rust::BatchSource::new(
                "dimensions/ArrayLengthSource.java",
                include_str!("fixtures/inherited_size_contract/ArrayLengthSource.java"),
            ),
            new_java2rust::BatchSource::new(
                "dimensions/ArrayLengthConsumer.java",
                include_str!("fixtures/inherited_size_contract/ArrayLengthConsumer.java"),
            ),
        ],
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ArrayLengthConsumer::run().value(), 0); }
"#,
    );
}

#[test]
fn numeric_text_temporary_staging_preserves_nullable_parser_abi_and_argument_order() {
    common::verify_java_fixture_zero_contract(
        "numeric_text_temporary",
        "NumericTemporaryTextProbe",
        "tests/fixtures/NumericTemporaryTextProbe.java",
        include_str!("fixtures/NumericTemporaryTextProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn parsing() { assert_eq!(NumericTemporaryTextProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn retained_element_reads_preserve_owned_carriers_nulls_and_bounds() {
    common::verify_java_fixture_zero_contract(
        "retained_element_read",
        "RetainedElementReadProbe",
        "tests/fixtures/RetainedElementReadProbe.java",
        include_str!("fixtures/RetainedElementReadProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reads() { assert_eq!(RetainedElementReadProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn shared_shallow_clone_copies_payload_state_and_retains_child_identity() {
    common::verify_java_fixture_zero_contract(
        "shared_shallow_clone",
        "SharedShallowCloneProbe",
        "tests/fixtures/SharedShallowCloneProbe.java",
        include_str!("fixtures/SharedShallowCloneProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(SharedShallowCloneProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn hierarchy_forwarders_copy_retained_handles_when_an_override_rebinds_a_parameter() {
    common::verify_java_fixture_zero_contract(
        "retained_override_argument",
        "RetainedOverrideArgumentProbe",
        "tests/fixtures/RetainedOverrideArgumentProbe.java",
        include_str!("fixtures/RetainedOverrideArgumentProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(RetainedOverrideArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn borrowed_array_views_keep_the_factory_carrier_backing_and_argument_order() {
    common::verify_java_fixture_zero_contract(
        "borrowed_array_view",
        "BorrowedArrayViewProbe",
        "tests/fixtures/BorrowedArrayViewProbe.java",
        include_str!("fixtures/BorrowedArrayViewProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn backing() { assert_eq!(BorrowedArrayViewProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn borrowed_factory_arguments_live_through_dispatch_and_preserve_receiver_order() {
    common::verify_java_fixture_zero_contract(
        "borrowed_temporary_argument",
        "BorrowedTemporaryArgumentProbe",
        "tests/fixtures/BorrowedTemporaryArgumentProbe.java",
        include_str!("fixtures/BorrowedTemporaryArgumentProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn dispatch() { assert_eq!(BorrowedTemporaryArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn receiver_eliminated_private_helpers_keep_the_emitted_array_parameter_contract() {
    common::verify_java_fixture_zero_contract(
        "private_helper_array_contract",
        "SourceArrayParameterContractProbe",
        "tests/fixtures/SourceArrayParameterContractProbe.java",
        include_str!("fixtures/SourceArrayParameterContractProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn contract() { assert_eq!(SourceArrayParameterContractProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn retained_collection_formatting_preserves_callbacks_nulls_and_exceptions() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_collection_formatting.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_collection_formatting",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(RetainedCollectionFormatting::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn nullable_generic_interface_results_are_checked_before_collection_copying() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/nullable_source_collection_results.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_source_collection_results",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(NullableSourceCollectionResults::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn physical_values_survive_borrowed_staging_mutable_iteration_and_copying() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/physical_call_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "physical_call_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(PhysicalCallValues::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn nested_source_calls_preserve_receiver_identity_argument_order_and_exceptions() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_call_staging.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_call_staging",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(SourceCallStaging::run().value(), 0); }
"#,
        }],
    );
}
