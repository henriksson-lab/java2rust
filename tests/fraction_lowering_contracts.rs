#[path = "common/mod.rs"]
mod common;

#[test]
fn mapped_numeric_conversions_keep_exact_values_rounding_and_errors() {
    let source = include_str!("corpus/numbers/fraction-lowering/NumericConversionsProbe.java");
    common::verify_java_sources_zero_contract(
        "numericconversions.NumericConversionsProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/NumericConversionsProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_numeric_conversions",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn conversions() { assert_eq!(NumericConversionsProbe::run().value(),0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_sign_magnitude_constructor_preserves_octets_snapshot_and_errors() {
    let source = include_str!("corpus/numbers/fraction-lowering/SignMagnitudeProbe.java");
    common::verify_java_sources_zero_contract(
        "signmagnitude.SignMagnitudeProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/SignMagnitudeProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_sign_magnitude",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn magnitude() { assert_eq!(SignMagnitudeProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_bytebuffer_magnitudes_use_the_backing_array_abi() {
    let source = include_str!("corpus/numbers/fraction-lowering/ByteBufferMagnitudeProbe.java");
    common::verify_java_sources_zero_contract(
        "signmagnitude.ByteBufferMagnitudeProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/ByteBufferMagnitudeProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_bytebuffer_magnitude",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn magnitude() { assert_eq!(ByteBufferMagnitudeProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn native_comparable_views_share_source_state_identity_and_ordering_contracts() {
    common::verify_java_fixture_zero_contract(
        "native_comparable",
        "nativecomparable.NativeComparableProbe",
        "tests/corpus/numbers/fraction-lowering/NativeComparableProbe.java",
        include_str!("corpus/numbers/fraction-lowering/NativeComparableProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn comparable() { assert_eq!(NativeComparableProbe::run().value(),0); }
"#,
    );
}

#[test]
fn nullable_native_suppliers_keep_java_owner_and_check_null_only_when_invoked() {
    common::verify_java_fixture_zero_contract(
        "nullable_native_supplier",
        "nullablenativesupplier.NullableNativeSupplierProbe",
        "tests/corpus/numbers/fraction-lowering/NullableNativeSupplierProbe.java",
        include_str!("corpus/numbers/fraction-lowering/NullableNativeSupplierProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn suppliers() { assert_eq!(NullableNativeSupplierProbe::run().value(),0); }
"#,
    );
}

#[test]
fn immutable_shared_erased_casts_agree_with_instanceof_and_preserve_aliases() {
    common::verify_java_fixture_zero_contract(
        "immutable_erased_cast",
        "immutableerasedcast.ImmutableErasedCastProbe",
        "tests/corpus/numbers/fraction-lowering/ImmutableErasedCastProbe.java",
        include_str!("corpus/numbers/fraction-lowering/ImmutableErasedCastProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn casts() { assert_eq!(ImmutableErasedCastProbe::run().value(),0); }
"#,
    );
}

#[test]
fn canonical_mapped_numeric_fields_retain_the_selected_provider_abi() {
    let source = include_str!("corpus/numbers/fraction-lowering/CanonicalMappedFieldsProbe.java");
    common::verify_java_sources_zero_contract(
        "CanonicalMappedFieldsCaller",
        &[
            (
                "tests/corpus/numbers/fraction-lowering/CanonicalMappedFieldsProbe.java",
                source,
            ),
            (
                "tests/corpus/numbers/fraction-lowering/CanonicalMappedFieldsCaller.java",
                include_str!("corpus/numbers/fraction-lowering/CanonicalMappedFieldsCaller.java"),
            ),
        ],
    );
    // Exercise manifest ingestion as a real corpus run does. Parsing facts
    // directly omits the CLI's Rust ABI discovery and can hide ABI overrides.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = common::run_with_watchdog(
        std::process::Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
            .arg("--external-pack-manifest")
            .arg(root.join("runtime/java-math/manifests/big-integer.facts"))
            .arg(
                root.join("tests/corpus/numbers/fraction-lowering/CanonicalMappedFieldsProbe.java"),
            ),
        std::time::Duration::from_secs(120),
    );
    assert!(
        output.status.is_some_and(|status| status.success()),
        "{}",
        output.stderr
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "canonical_mapped_numeric_fields",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: output.stdout,
            tests: r#"
#[test] fn canonical_mapped_fields() {
    use java_math_runtime::BigInteger as Integer;
    use std::sync::Arc;
    let huge = Integer::value_of(2).pow(100).unwrap();
    let left = Arc::new(CanonicalMappedFieldsProbe::new(huge.clone(), Some(Integer::value_of(3))));
    let right = Arc::new(CanonicalMappedFieldsProbe::new(Integer::value_of(5), Some(Integer::value_of(7))));
    for _ in 0..3 {
        let product = left.multiply_canonical_mapped_fields_probe(Arc::clone(&right)).unwrap();
        assert_eq!(product.numerator.as_ref().unwrap(), &huge.multiply(Integer::value_of(5)).unwrap());
        assert_eq!(product.denominator.as_ref().unwrap().int_value(), 21);
        assert_eq!(left.multiply_int(2).unwrap().numerator.as_ref().unwrap(), &huge.add(&huge).unwrap());
        assert_eq!(left.multiply_big_integer(&Integer::value_of(2)).unwrap().numerator.as_ref().unwrap(), &huge.add(&huge).unwrap());
    }
    assert!(Arc::ptr_eq(&CanonicalMappedFieldsProbe::identity_with_outer(Arc::clone(&left)), &left));
    let zero = CanonicalMappedFieldsProbe::__static_ZERO();
    assert!(Arc::ptr_eq(&CanonicalMappedFieldsProbe::identity_with_outer(Arc::clone(&zero)), &zero));
    assert_eq!(zero.multiply_canonical_mapped_fields_probe(right).unwrap().numerator.as_ref().unwrap().signum(), 0);
    assert_eq!(left.numerator.as_ref().unwrap(), &huge);
    assert_eq!(left.denominator.as_ref().unwrap().int_value(), 3);
    let nullable = CanonicalMappedFieldsProbe::new(Integer::value_of(1), None);
    assert!(nullable.denominator.is_none());
}
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_numeric_fields_keep_call_contracts_and_reusable_payloads() {
    let source = include_str!("corpus/numbers/fraction-lowering/MappedNumericFieldsProbe.java");
    common::verify_java_sources_zero_contract(
        "mappednumericfields.MappedNumericFieldsProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/MappedNumericFieldsProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_numeric_fields",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn mapped_fields() { assert_eq!(MappedNumericFieldsProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn immutable_interface_forwarders_keep_canonical_self_and_inherited_generic_slots() {
    common::verify_java_fixture_zero_contract(
        "immutable_interface_identity",
        "immutableinterfaceidentity.ImmutableInterfaceIdentityProbe",
        "tests/corpus/numbers/fraction-lowering/ImmutableInterfaceIdentityProbe.java",
        include_str!("corpus/numbers/fraction-lowering/ImmutableInterfaceIdentityProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn interface_identity() { assert_eq!(ImmutableInterfaceIdentityProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn mapped_integer_parsing_keeps_java_unicode_diagnostics_and_values() {
    let source = include_str!("corpus/numbers/fraction-lowering/BigIntegerParseProbe.java");
    common::verify_java_sources_zero_contract(
        "integerparse.BigIntegerParseProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/BigIntegerParseProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_integer_parse",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn parse_contracts() { assert_eq!(BigIntegerParseProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_decimal_division_preserves_constructor_overloads_scale_and_errors() {
    let source = include_str!("corpus/numbers/fraction-lowering/DecimalDivisionProbe.java");
    common::verify_java_sources_zero_contract(
        "decimaldivision.DecimalDivisionProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/DecimalDivisionProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_decimal_division",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn decimal_division_contracts() { assert_eq!(DecimalDivisionProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn mapped_integer_arithmetic_keeps_precision_borrows_and_java_errors() {
    let source = include_str!("corpus/numbers/fraction-lowering/BigIntegerArithmeticProbe.java");
    common::verify_java_sources_zero_contract(
        "integerarithmetic.BigIntegerArithmeticProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/BigIntegerArithmeticProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_integer_arithmetic",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn exact_numeric_contracts() { assert_eq!(BigIntegerArithmeticProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}

#[test]
fn stateful_anonymous_suppliers_retain_virtual_outer_dispatch_and_aliases() {
    common::verify_java_fixture_zero_contract(
        "stateful_outer_supplier",
        "statefuloutersupplier.StatefulOuterSupplierProbe",
        "tests/corpus/numbers/fraction-lowering/StatefulOuterSupplierProbe.java",
        include_str!("corpus/numbers/fraction-lowering/StatefulOuterSupplierProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn state_and_virtual_dispatch() {
    assert_eq!(StatefulOuterSupplierProbe::run().value(), 0);
}
"#,
    );
}

#[test]
fn anonymous_object_fields_use_java_primitive_defaults_and_shared_state() {
    common::verify_java_fixture_zero_contract(
        "anonymous_primitive_defaults",
        "anonymousprimitivedefaults.AnonymousPrimitiveDefaultsProbe",
        "tests/corpus/numbers/fraction-lowering/AnonymousPrimitiveDefaultsProbe.java",
        include_str!("corpus/numbers/fraction-lowering/AnonymousPrimitiveDefaultsProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn defaults_and_aliases() { assert_eq!(AnonymousPrimitiveDefaultsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn native_anonymous_sams_share_state_and_keep_callback_exception_order() {
    common::verify_java_fixture_zero_contract(
        "stateful_native_sam",
        "statefulnativesam.StatefulNativeSamProbe",
        "tests/corpus/numbers/fraction-lowering/StatefulNativeSamProbe.java",
        include_str!("corpus/numbers/fraction-lowering/StatefulNativeSamProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn stateful_sams() { assert_eq!(StatefulNativeSamProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn mapped_numeric_references_preserve_identity_across_fields_and_returns() {
    let source = include_str!("corpus/numbers/fraction-lowering/MappedNumericIdentityProbe.java");
    common::verify_java_sources_zero_contract(
        "mappednumericidentity.MappedNumericIdentityProbe",
        &[(
            "tests/corpus/numbers/fraction-lowering/MappedNumericIdentityProbe.java",
            source,
        )],
    );
    let pack = new_java2rust::external_packs::parse_external_pack_manifest(include_str!(
        "../runtime/java-math/manifests/big-integer.facts"
    ));
    assert!(!pack.diagnostics().has_errors(), "{:?}", pack.diagnostics());
    let translation = new_java2rust::translate_source_with_facts(source, pack.facts());
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/java-math");
    common::cargo_verify_generated_modules_with_dependencies(
        "mapped_numeric_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn retained_identity() { assert_eq!(MappedNumericIdentityProbe::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
        &[("java-math-runtime", runtime.as_path())],
    );
}
