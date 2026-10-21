#[path = "common/mod.rs"]
mod common;

#[test]
fn checked_constructor_initializes_trait_payloads_after_validation_in_order() {
    let translation = new_java2rust::translate_source(
        r#"
interface Operation { int apply(); }
class Events {
    static int count;
}
class FirstOperation implements Operation {
    FirstOperation() { Events.count = Events.count * 10 + 1; }
    public int apply() { return 3; }
}
class SecondOperation implements Operation {
    SecondOperation() { Events.count = Events.count * 10 + 2; }
    public int apply() { return 5; }
}
class ValidatedOperations {
    final Operation first;
    final Operation second;
    ValidatedOperations(int size) {
        if (size < 0) throw new IllegalArgumentException("negative");
        int checked = size + 1;
        first = new FirstOperation();
        if (checked == 0) throw new IllegalArgumentException("overflow");
        second = new SecondOperation();
    }
    int apply() { return first.apply() + second.apply(); }
    static int run() {
        try {
            new ValidatedOperations(-1);
            return 1;
        } catch (IllegalArgumentException expected) {
            if (Events.count != 0) return 2;
        }
        ValidatedOperations operations = new ValidatedOperations(2);
        if (Events.count != 12) return 3;
        if (operations.apply() != 8) return 4;
        return 0;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "sampling_constructor_validation",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn ordered_validation() { assert_eq!(ValidatedOperations::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}
