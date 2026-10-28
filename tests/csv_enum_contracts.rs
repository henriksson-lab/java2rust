#[path = "common/mod.rs"]
mod common;
#[test]
fn enum_payload_accessors_copy_handles_and_chain_checked_value_of() {
    common::verify_java_fixture_zero_contract(
        "csv_enum_handle",
        "EnumHandleContracts",
        "tests/corpus/csv/EnumHandleContracts.java",
        include_str!("corpus/csv/EnumHandleContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn enum_contract() {
    assert_eq!(EnumHandleContracts::run().value(), 0);
    erased_enum_reference_projects_the_original_live_constant();
}
fn erased_enum_reference_projects_the_original_live_constant() {
    type Choice = EnumHandleContracts_Choice;
    let first = JavaEnumReference::from_source(&Choice::A);
    let copy = first.clone();
    assert_eq!(first, copy);
    assert_ne!(first, JavaEnumReference::from_source(&Choice::B));
    assert_eq!(first.identity(), JavaReferenceIdentity::java_reference_identity(&Choice::A) as usize);
    assert!(first.name().java_equals(&JavaString::intern_literal("A")));
    assert_eq!(first.ordinal(), 0);
    assert_eq!(first.compare_to_result(&JavaEnumReference::from_source(&Choice::B)).unwrap(), -1);
    Choice::A.visit();
    let projected: Choice = copy.cast_result().unwrap();
    assert!(projected.seen() > 0);
    assert!(copy.cast_result::<JavaString>().is_err());
    assert!(copy.to_string_result().unwrap().unwrap().java_equals(&JavaString::intern_literal("A")));
    let class = Choice::A.get_declaring_class().retarget::<()>();
    let mut first = class.enum_reference_constants_result().unwrap().unwrap();
    let second = class.clone().enum_reference_constants_result().unwrap().unwrap();
    assert_eq!(first[0], second[0]);
    first.remove(0);
    assert_eq!(second[0], copy);
    assert!(JavaClass::<JavaString>::literal("java.lang.String").enum_reference_constants_result().unwrap().is_none());
    assert!(JavaClass::<()>::literal("unknown.ExternalEnum").enum_reference_constants_result().is_err());
    fn send_sync<T: Send + Sync>() {}
    send_sync::<JavaEnumReference>();
}
"#,
    );
}

#[test]
fn qualified_enum_lookup_uses_resolved_owner_in_batch_emission() {
    let source = include_str!("corpus/csv/EnumLookupContracts.java");
    let peer = include_str!("corpus/csv/EnumMetadataPeer.java");
    common::verify_java_sources_zero_contract(
        "probe.enums.EnumLookupContracts",
        &[
            ("tests/corpus/csv/EnumLookupContracts.java", source),
            ("tests/corpus/csv/EnumMetadataPeer.java", peer),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "csv_qualified_enum_lookup",
        &[
            new_java2rust::BatchSource::new("EnumLookupContracts.java", source),
            new_java2rust::BatchSource::new("EnumMetadataPeer.java", peer),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() {
    assert_eq!(translated::EnumLookupContracts::run().value(), 0);
    let token = translated::EnumMetadataPeer::token();
    assert_eq!(translated::EnumMetadataPeer::initialized_count(), 0);
    let callbacks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = callbacks.clone();
    let during_initialization = token.clone();
    translated::EnumMetadataPeer::__set_static_callback(Some(translated::__JavaSyncRunnable::from_fn(move || {
        assert!(during_initialization.enum_reference_constants_result().unwrap().is_none());
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    })));
    let first = token.enum_reference_constants_result().unwrap().unwrap();
    assert_eq!(translated::EnumMetadataPeer::initialized_count(), 1);
    assert_eq!(callbacks.load(std::sync::atomic::Ordering::SeqCst), 1);
    let second = token.clone().enum_reference_constants_result().unwrap().unwrap();
    assert_eq!(translated::EnumMetadataPeer::initialized_count(), 1);
    assert_eq!(first[0], second[0]);
}
"#,
    );
}
