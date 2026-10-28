#[path = "common/mod.rs"]
mod common;

#[test]
fn closing_stream_aliases_and_linked_stages_invalidates_without_traversal() {
    common::verify_java_fixture_zero_contract(
        "csv_stream_lifecycle",
        "StreamLifecycleContracts",
        "tests/corpus/csv/StreamLifecycleContracts.java",
        include_str!("corpus/csv/StreamLifecycleContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lifecycle() { assert_eq!(StreamLifecycleContracts::run().value(), 0); }
"#,
    );
}

#[test]
fn stored_stream_stage_contracts_survive_production_assembly_and_source_calls() {
    let source = include_str!("corpus/csv/StreamLifecycleContracts.java");
    let peer = include_str!("corpus/csv/StreamLifecyclePeer.java");
    common::verify_java_sources_zero_contract(
        "StreamLifecyclePeer",
        &[
            ("tests/corpus/csv/StreamLifecycleContracts.java", source),
            ("tests/corpus/csv/StreamLifecyclePeer.java", peer),
        ],
    );
    common::cargo_run_translated_batch_sources(
        "csv_stream_lifecycle_batch",
        &[
            new_java2rust::BatchSource::new("StreamLifecycleContracts.java", source),
            new_java2rust::BatchSource::new("StreamLifecyclePeer.java", peer),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::StreamLifecyclePeer::run().value(), 0); }
"#,
    );
}
