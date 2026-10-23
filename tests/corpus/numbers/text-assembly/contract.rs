use new_java2rust_focused_output::{JavaString, TextAssemblyClient};

trait ExitCode {
    fn value(self) -> i32;
}
impl ExitCode for i32 {
    fn value(self) -> i32 {
        self
    }
}
impl<E: std::fmt::Debug> ExitCode for Result<i32, E> {
    fn value(self) -> i32 {
        self.unwrap()
    }
}

#[test]
fn complete_assembled_modules_preserve_text_values_and_alias_impls() {
    assert_eq!(TextAssemblyClient::run().value(), 0);
    let native: JavaString = "é".to_string().into();
    assert_eq!(native.units(), &[233]);
    let borrowed: JavaString = "abc".into();
    assert_eq!(borrowed.units(), &[97, 98, 99]);
}
