#[path = "common/mod.rs"]
mod common;
#[test]
fn character_text_producers_preserve_units_codepoints_and_exceptions() {
    common::verify_java_fixture_zero_contract(
        "csv_character_text",
        "CharacterTextContracts",
        "tests/corpus/csv/CharacterTextContracts.java",
        include_str!("corpus/csv/CharacterTextContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn character_text() { assert_eq!(CharacterTextContracts::run().value(), 0); }
"#,
    );
}
