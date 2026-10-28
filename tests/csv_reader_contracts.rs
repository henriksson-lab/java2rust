#[path = "common/mod.rs"]
mod common;

#[test]
fn reader_bulk_units_aliases_and_source_override_dispatch_match_java() {
    common::verify_java_fixture_zero_contract(
        "csv_reader_contracts",
        "ReaderContracts",
        "tests/corpus/csv/ReaderContracts.java",
        include_str!("corpus/csv/ReaderContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reader_contract() { assert_eq!(ReaderContracts::run().value(), 0); }
#[test] fn synchronized_array_and_scalar_interop() {
    let reader = Reader::from_string(JavaUtf16::from_units(vec![0xd800, 120]));
    let array = std::sync::Arc::new(std::sync::Mutex::new(vec![JavaCodeUnit(0); 3]));
    assert_eq!(reader.read_3(Some(&array), 1, 2).unwrap(), 2);
    assert_eq!(*array.lock().unwrap(), vec![JavaCodeUnit(0), JavaCodeUnit(0xd800), JavaCodeUnit(120)]);
    assert_eq!(reader.read_3(Some(&array), 0, 1).unwrap(), -1);
    assert_eq!(reader.read_3(Some(&array), 0, 0).unwrap(), 0);
    reader.close().unwrap();
    assert!(reader.read_3(Some(&array), -1, 1).unwrap_err().is_instance(&["IOException"]));
    let native = Reader::from_string("\u{1f600}");
    let mut scalar = vec!['\0'];
    assert_eq!(native.read(scalar.as_mut_slice()).unwrap(), 1);
    assert_eq!(scalar, vec!['\u{1f600}']);
}
"#,
    );
}
