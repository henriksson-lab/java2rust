#[path = "common/mod.rs"]
mod common;

#[test]
fn stored_object_arrays_retain_covariant_owners_across_fields_and_calls() {
    common::verify_java_fixture_zero_contract(
        "csv_stored_array_view",
        "StoredArrayViewContracts",
        "tests/corpus/csv/StoredArrayViewContracts.java",
        include_str!("corpus/csv/StoredArrayViewContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copy_contract() { assert_eq!(StoredArrayViewContracts::run().value(), 0); }
"#,
    );
}

#[test]
fn stored_object_array_views_cross_separately_emitted_source_calls() {
    let primary = include_str!("corpus/csv/StoredArrayViewContracts.java");
    let peer = include_str!("corpus/csv/StoredArrayViewPeer.java");
    common::verify_java_sources_zero_contract(
        "StoredArrayViewPeer",
        &[
            ("tests/corpus/csv/StoredArrayViewContracts.java", primary),
            ("tests/corpus/csv/StoredArrayViewPeer.java", peer),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "csv_stored_array_view_batch",
        &[
            new_java2rust::BatchSource::new("StoredArrayViewContracts.java", primary),
            new_java2rust::BatchSource::new("StoredArrayViewPeer.java", peer),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() {
    assert_eq!(translated::StoredArrayViewContracts::run().value(), 0);
    assert_eq!(translated::StoredArrayViewPeer::run().value(), 0);
}
"#,
    );
}
