#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_array_list_views_retain_covariant_owners_and_checked_stores() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_array_list_view",
        "ErasedArrayListViewContracts",
        "tests/corpus/csv/ErasedArrayListViewContracts.java",
        include_str!("corpus/csv/ErasedArrayListViewContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copy_contract() { assert_eq!(ErasedArrayListViewContracts::run().value(), 0); }
"#,
    );
}

#[test]
fn erased_array_list_views_survive_batch_support_assembly() {
    let peer = include_str!("corpus/csv/ErasedArrayListViewPeer.java");
    common::verify_java_sources_zero_contract(
        "ErasedArrayListViewPeer",
        &[
            (
                "tests/corpus/csv/ErasedArrayListViewContracts.java",
                include_str!("corpus/csv/ErasedArrayListViewContracts.java"),
            ),
            ("tests/corpus/csv/ErasedArrayListViewPeer.java", peer),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "csv_erased_array_list_view_batch",
        &[
            new_java2rust::BatchSource::new(
                "ErasedArrayListViewContracts.java",
                include_str!("corpus/csv/ErasedArrayListViewContracts.java"),
            ),
            new_java2rust::BatchSource::new("ErasedArrayListViewPeer.java", peer),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() {
    assert_eq!(translated::ErasedArrayListViewContracts::run().value(), 0);
    assert_eq!(translated::ErasedArrayListViewPeer::run().value(), 0);
}
"#,
    );
}
